//! The stack model (design section 7).
//!
//! A stack is a tree of branches on one trunk. Each branch has exactly one parent:
//! the trunk or another branch of the stack. A chain is the common case, but
//! a branch can have more than one child.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{BranchName, ObjectId, PrNumber};

/// One branch of a stack, as kept in `refs/grafene/meta/<branch>` (design section 7.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub name: BranchName,
    pub parent: BranchName,
    /// The parent commit at the last restack. Restack compares it with the
    /// current parent tip (design section 7.2, step 2).
    pub parent_base: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrNumber>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StackError {
    #[error("branch {0} occurs more than one time")]
    DuplicateBranch(BranchName),

    #[error("branch {0} has the same name as the trunk")]
    TrunkInStack(BranchName),

    #[error("branch {branch} has parent {parent}, which is not the trunk and not in the stack")]
    UnknownParent { branch: BranchName, parent: BranchName },

    #[error("branch {0} is in a parent cycle and does not connect to the trunk")]
    Cycle(BranchName),
}

/// A validated tree of branches on one trunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    trunk: BranchName,
    branches: BTreeMap<BranchName, Branch>,
    /// Parent name to child names. Children are sorted by name.
    children: BTreeMap<BranchName, Vec<BranchName>>,
    /// All branches, parents before children. Siblings are sorted by name.
    order: Vec<BranchName>,
}

impl Stack {
    /// Make a stack and validate it.
    ///
    /// # Errors
    ///
    /// Returns an error if a name occurs two times, a branch has the trunk name,
    /// a parent is unknown, or the parent links make a cycle.
    pub fn new(trunk: BranchName, branches: impl IntoIterator<Item = Branch>) -> Result<Self, StackError> {
        let mut map = BTreeMap::new();
        for b in branches {
            if b.name == trunk {
                return Err(StackError::TrunkInStack(b.name));
            }
            if let Some(old) = map.insert(b.name.clone(), b) {
                return Err(StackError::DuplicateBranch(old.name));
            }
        }

        let mut children: BTreeMap<BranchName, Vec<BranchName>> = BTreeMap::new();
        for b in map.values() {
            if b.parent != trunk && !map.contains_key(&b.parent) {
                return Err(StackError::UnknownParent { branch: b.name.clone(), parent: b.parent.clone() });
            }
            // `map` iterates in name order, so each child list is sorted.
            children.entry(b.parent.clone()).or_default().push(b.name.clone());
        }

        // Each branch has one parent. Thus a branch that the walk from trunk
        // does not reach is in a cycle (or above a cycle).
        let mut order = Vec::with_capacity(map.len());
        let mut todo: Vec<&BranchName> = children.get(&trunk).map(|c| c.iter().rev().collect()).unwrap_or_default();
        while let Some(name) = todo.pop() {
            order.push(name.clone());
            if let Some(c) = children.get(name) {
                todo.extend(c.iter().rev());
            }
        }
        if order.len() != map.len() {
            let reached: BTreeSet<&BranchName> = order.iter().collect();
            let first = map.keys().find(|n| !reached.contains(n)).cloned();
            return Err(StackError::Cycle(first.expect("a branch is not reached")));
        }

        Ok(Self { trunk, branches: map, children, order })
    }

    #[must_use]
    pub const fn trunk(&self) -> &BranchName {
        &self.trunk
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.branches.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    #[must_use]
    pub fn get(&self, name: &BranchName) -> Option<&Branch> {
        self.branches.get(name)
    }

    /// The direct children of `name`, sorted by name. `name` can be the trunk.
    pub fn children(&self, name: &BranchName) -> impl Iterator<Item = &Branch> {
        self.children.get(name).into_iter().flatten().map(|n| &self.branches[n])
    }

    /// The branches below `name`, nearest first. The trunk is not included.
    /// Empty if `name` is not in the stack.
    #[must_use]
    pub fn ancestors(&self, name: &BranchName) -> Vec<&Branch> {
        let mut out = Vec::new();
        let mut cur = self.branches.get(name).map(|b| &b.parent);
        while let Some(b) = cur.and_then(|p| self.branches.get(p)) {
            out.push(b);
            cur = Some(&b.parent);
        }
        out
    }

    /// `name` and all branches above it, parents before children.
    /// Restack uses this order after a change to `name`.
    #[must_use]
    pub fn descendants(&self, name: &BranchName) -> Vec<&Branch> {
        let mut out = Vec::new();
        let Some(start) = self.branches.get(name) else { return out };
        let mut todo = vec![start];
        while let Some(b) = todo.pop() {
            out.push(b);
            let mut c: Vec<&Branch> = self.children(&b.name).collect();
            c.reverse();
            todo.extend(c);
        }
        out
    }

    /// All branches, parents before children (design section 7.2, step 1).
    /// Siblings are sorted by name, so the order is deterministic.
    pub fn topo_order(&self) -> impl ExactSizeIterator<Item = &Branch> {
        self.order.iter().map(|n| &self.branches[n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> BranchName {
        s.parse().unwrap()
    }

    fn b(name: &str, parent: &str) -> Branch {
        Branch { name: n(name), parent: n(parent), parent_base: "0".repeat(40).parse().unwrap(), pr: None }
    }

    fn names<'a>(it: impl IntoIterator<Item = &'a Branch>) -> Vec<&'a str> {
        it.into_iter().map(|b| b.name.as_str()).collect()
    }

    /// main ── a ── b ── d
    ///          └── c
    /// main ── x
    fn tree() -> Stack {
        Stack::new(n("main"), [b("d", "b"), b("c", "a"), b("b", "a"), b("a", "main"), b("x", "main")]).unwrap()
    }

    #[test]
    fn topo_order_is_parents_first_and_deterministic() {
        assert_eq!(names(tree().topo_order()), ["a", "b", "d", "c", "x"]);
    }

    #[test]
    fn children_ancestors_descendants() {
        let s = tree();
        assert_eq!(names(s.children(&n("main"))), ["a", "x"]);
        assert_eq!(names(s.children(&n("a"))), ["b", "c"]);
        assert_eq!(names(s.ancestors(&n("d"))), ["b", "a"]);
        assert!(s.ancestors(&n("a")).is_empty());
        assert_eq!(names(s.descendants(&n("a"))), ["a", "b", "d", "c"]);
        assert!(s.descendants(&n("main")).is_empty());
        assert!(s.descendants(&n("nope")).is_empty());
    }

    #[test]
    fn long_chain_restacks_in_order() {
        let chain: Vec<Branch> = (0..20)
            .map(|i| b(&format!("s{i:02}"), &if i == 0 { "main".into() } else { format!("s{:02}", i - 1) }))
            .collect();
        let s = Stack::new(n("main"), chain.into_iter().rev()).unwrap();
        let order = names(s.topo_order());
        let expected: Vec<String> = (0..20).map(|i| format!("s{i:02}")).collect();
        assert_eq!(order, expected);
        assert_eq!(s.descendants(&n("s15")).len(), 5);
    }

    #[test]
    fn empty_stack() {
        let s = Stack::new(n("main"), []).unwrap();
        assert!(s.is_empty());
        assert_eq!(s.topo_order().len(), 0);
    }

    #[test]
    fn rejects_duplicate() {
        let e = Stack::new(n("main"), [b("a", "main"), b("a", "main")]).unwrap_err();
        assert_eq!(e, StackError::DuplicateBranch(n("a")));
    }

    #[test]
    fn rejects_trunk_as_branch() {
        let e = Stack::new(n("main"), [b("main", "a")]).unwrap_err();
        assert_eq!(e, StackError::TrunkInStack(n("main")));
    }

    #[test]
    fn rejects_unknown_parent() {
        let e = Stack::new(n("main"), [b("a", "ghost")]).unwrap_err();
        assert_eq!(e, StackError::UnknownParent { branch: n("a"), parent: n("ghost") });
    }

    #[test]
    fn rejects_cycle() {
        let e = Stack::new(n("main"), [b("a", "main"), b("p", "q"), b("q", "p")]).unwrap_err();
        assert_eq!(e, StackError::Cycle(n("p")));
        let e = Stack::new(n("main"), [b("self", "self")]).unwrap_err();
        assert_eq!(e, StackError::Cycle(n("self")));
    }

    #[test]
    fn branch_serde_omits_missing_pr() {
        let json = serde_json::to_string(&b("a", "main")).unwrap();
        assert!(!json.contains("\"pr\""));
        let mut with_pr = b("a", "main");
        with_pr.pr = Some(PrNumber(7));
        let back: Branch = serde_json::from_str(&serde_json::to_string(&with_pr).unwrap()).unwrap();
        assert_eq!(back, with_pr);
    }
}
