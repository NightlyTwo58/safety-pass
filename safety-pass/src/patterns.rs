/*!

Simple cell patterns.

*/

use crate::{Cell, CellType, Create, Pattern, Primitive, Replace};
use log::debug;
use safety_net::{DrivenNet, Error, Instantiable, Logic, NetRef};
use std::fmt;

// Boolean operation abstraction

/// Logical class of a Boolean gate, independent of arity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BooleanOp {
    And,
    Or,
    Nand,
    Nor,
    Xor,
    Xnor,
}

impl BooleanOp {
    fn from_type(ct: CellType) -> Option<Self> {
        match ct {
            CellType::AND | CellType::AND2 | CellType::AND3 | CellType::AND4 => Some(Self::And),
            CellType::OR | CellType::OR2 | CellType::OR3 | CellType::OR4 => Some(Self::Or),
            CellType::NAND | CellType::NAND2 | CellType::NAND3 | CellType::NAND4 => {
                Some(Self::Nand)
            }
            CellType::NOR | CellType::NOR2 | CellType::NOR3 | CellType::NOR4 => Some(Self::Nor),
            CellType::XOR | CellType::XOR2 => Some(Self::Xor),
            CellType::XNOR | CellType::XNOR2 => Some(Self::Xnor),
            _ => None,
        }
    }

    fn is_monotone(self) -> bool {
        matches!(self, Self::And | Self::Or)
    }

    /// The identity input value: the one that leaves the other input unchanged.
    ///
    /// AND/NAND: 1 (AND(A,1)=A, NAND(A,1)=NOT(A))
    /// OR/NOR:   0 (OR(A,0)=A,  NOR(A,0)=NOT(A))
    /// XOR:      0 (XOR(A,0)=A)
    /// XNOR:     1 (XNOR(A,1)=A)
    fn identity(self) -> Logic {
        match self {
            Self::And | Self::Nand | Self::Xnor => Logic::True,
            Self::Or | Self::Nor | Self::Xor => Logic::False,
        }
    }

    /// The absorbing input value for monotone and negated-monotone gates.
    /// Returns `None` for XOR/XNOR, which have no absorbing element.
    ///
    /// The value returned is the *input* that triggers absorption, not the output.
    /// AND:  absorbing input = 0  → output = 0
    /// OR:   absorbing input = 1  → output = 1
    /// NAND: absorbing input = 0  → output = 1
    /// NOR:  absorbing input = 1  → output = 0
    fn absorbing_input(self) -> Option<Logic> {
        match self {
            Self::And | Self::Nand => Some(Logic::False),
            Self::Or | Self::Nor => Some(Logic::True),
            Self::Xor | Self::Xnor => None,
        }
    }

    /// Output produced when an absorbing input is present.
    fn absorbing_output(self) -> Option<Logic> {
        match self {
            Self::And => Some(Logic::False),
            Self::Or => Some(Logic::True),
            Self::Nand => Some(Logic::True),
            Self::Nor => Some(Logic::False),
            Self::Xor | Self::Xnor => None,
        }
    }
}

// Gate-type sizing helpers

/// Maps `(op, arity)` to an explicitly-sized CellType, e.g. (And, 3) → AND3.
/// Returns `None` for unsupported arities or ops (e.g. XOR has no 3-input form).
fn boolean_type(op: BooleanOp, inputs: usize) -> Option<CellType> {
    match (op, inputs) {
        (BooleanOp::And, 2) => Some(CellType::AND2),
        (BooleanOp::And, 3) => Some(CellType::AND3),
        (BooleanOp::And, 4) => Some(CellType::AND4),
        (BooleanOp::Or, 2) => Some(CellType::OR2),
        (BooleanOp::Or, 3) => Some(CellType::OR3),
        (BooleanOp::Or, 4) => Some(CellType::OR4),
        (BooleanOp::Nand, 2) => Some(CellType::NAND2),
        (BooleanOp::Nand, 3) => Some(CellType::NAND3),
        (BooleanOp::Nand, 4) => Some(CellType::NAND4),
        (BooleanOp::Nor, 2) => Some(CellType::NOR2),
        (BooleanOp::Nor, 3) => Some(CellType::NOR3),
        (BooleanOp::Nor, 4) => Some(CellType::NOR4),
        _ => None,
    }
}

/// Like `boolean_type`, but preserves generic (unsized) cell types when the
/// original cell was already unsized and the new arity is 2.
///
/// For example: shrinking AND3 to 2 inputs gives AND2, but shrinking a generic
/// AND (unsized) to 2 inputs keeps AND rather than AND2, preserving whatever
/// semantic distinction the front-end attached to the unsized form.
fn resized_boolean_type(original: CellType, inputs: usize) -> Option<CellType> {
    let op = BooleanOp::from_type(original)?;
    if inputs == 2 {
        let unsized_form = match (op, original) {
            (BooleanOp::And, CellType::AND) => Some(CellType::AND),
            (BooleanOp::Or, CellType::OR) => Some(CellType::OR),
            (BooleanOp::Nand, CellType::NAND) => Some(CellType::NAND),
            (BooleanOp::Nor, CellType::NOR) => Some(CellType::NOR),
            _ => None,
        };
        if let Some(ct) = unsized_form {
            return Some(ct);
        }
    }
    boolean_type(op, inputs)
}

/// Maps AND/AND2 → NAND/NAND2, OR/OR2 → NOR/NOR2.
/// Used by `NotBooleanMap` to merge NOT(AND(A,B)) → NAND(A,B).
fn negated_map_type(ct: CellType) -> Option<CellType> {
    match ct {
        CellType::AND => Some(CellType::NAND),
        CellType::AND2 => Some(CellType::NAND2),
        CellType::OR => Some(CellType::NOR),
        CellType::OR2 => Some(CellType::NOR2),
        _ => None,
    }
}

// Shared structural helpers

/// Returns `true` if `b` is an inverter whose sole input is `a`.
/// Used to detect complementary-input pairs (A, NOT(A)).
fn is_inverted_of(a: &DrivenNet<Cell>, b: &DrivenNet<Cell>) -> bool {
    if !matches!(b.get_ptype(), Some(CellType::INV) | Some(CellType::NOT)) {
        return false;
    }
    let b_ref = b.clone().unwrap();
    let Some(inner) = b_ref.get_input(0).get_driver() else {
        return false;
    };
    inner == *a
}

// Pattern: BooleanSimplify

/// Constant propagation, absorbing-element, identity-element, and
/// equal-input simplification for all Boolean gates (AND/OR/NAND/NOR through
/// 4 inputs; XOR/XNOR at 2 inputs).
///
/// Replaces the former AndAbsorb, AndIdentity, OrAbsorb, OrIdentity,
/// NandAbsorb, NandIdentity, NorAbsorb, NorIdentity, and Idempotent patterns,
/// and extends their coverage to AND3/AND4/OR3/OR4/NAND3/NAND4/NOR3/NOR4.
#[derive(Debug)]
pub struct BooleanSimplify;

impl fmt::Display for BooleanSimplify {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Boolean constant/identity/absorb/idempotent simplification"
        )
    }
}

impl Pattern for BooleanSimplify {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        let Some(op) = BooleanOp::from_type(ct) else {
            return Ok(false);
        };

        // Collect all drivers; bail if any input is undriven.
        let mut inputs: Vec<DrivenNet<Cell>> = Vec::new();
        for port in cell.inputs() {
            let Some(driver) = port.get_driver() else {
                return Ok(false);
            };
            inputs.push(driver);
        }

        let constants: Vec<Option<Logic>> = inputs
            .iter()
            .map(|d| d.get_instance_type().and_then(|t| t.get_constant()))
            .collect();

        let output = cell.get_output(0);
        let inst_name = cell.get_instance_name().unwrap();

        //  All inputs are constants: evaluate completely
        if constants.iter().all(Option::is_some) {
            let vals: Vec<Logic> = constants.into_iter().map(Option::unwrap).collect();
            let result = match op {
                BooleanOp::And => vals.iter().all(|v| *v == Logic::True),
                BooleanOp::Or => vals.contains(&Logic::True),
                BooleanOp::Nand => !vals.iter().all(|v| *v == Logic::True),
                BooleanOp::Nor => !vals.contains(&Logic::True),
                BooleanOp::Xor => vals.iter().filter(|v| **v == Logic::True).count() % 2 == 1,
                BooleanOp::Xnor => vals.iter().filter(|v| **v == Logic::True).count() % 2 == 0,
            };
            let constant = create(
                cell_type.new_like(if result { CellType::VCC } else { CellType::GND }),
                inst_name + "_const".into(),
            );
            debug!("BooleanSimplify: constant-folded {}!", output.as_net());
            replace(output, constant.get_output(0))?;
            return Ok(true);
        }

        // XOR/XNOR are only represented as binary cells; multi-input forms
        // cannot arise, but guard here defensively.
        if matches!(op, BooleanOp::Xor | BooleanOp::Xnor) && inputs.len() != 2 {
            return Ok(false);
        }

        //  Absorbing element: one constant triggers a fixed output
        if let Some(absorbing) = op.absorbing_input()
            && constants.contains(&Some(absorbing))
        {
            let result = op.absorbing_output().unwrap();
            let constant = create(
                cell_type.new_like(if result == Logic::True {
                    CellType::VCC
                } else {
                    CellType::GND
                }),
                inst_name + "_const".into(),
            );
            debug!("BooleanSimplify: absorbing element on {}!", output.as_net());
            replace(output, constant.get_output(0))?;
            return Ok(true);
        }

        //  XOR/XNOR inverting constant: the non-identity constant on a
        //  2-input XOR/XNOR turns the gate into an inverter of the other
        //  input (XOR(A,1)=NOT(A), XNOR(A,0)=NOT(A)).
        if matches!(op, BooleanOp::Xor | BooleanOp::Xnor) && inputs.len() == 2 {
            let identity = op.identity();
            let non_identity_const = constants
                .iter()
                .position(|c| matches!(c, Some(v) if *v != identity));
            if let Some(idx) = non_identity_const {
                // The other input must be non-constant; if it were also
                // constant we'd have hit the all-constants branch above.
                let other = inputs[1 - idx].clone();
                let inv = create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
                inv.get_input(0).connect(other);
                debug!(
                    "BooleanSimplify: XOR/XNOR inverting constant reduced to INV on {}!",
                    output.as_net()
                );
                replace(output, inv.get_output(0))?;
                return Ok(true);
            }
        }

        //  Identity elements: strip them, resize or reduce the gate
        let identity = op.identity();
        let remaining: Vec<DrivenNet<Cell>> = inputs
            .iter()
            .zip(constants.iter())
            .filter(|(_, c)| **c != Some(identity))
            .map(|(d, _)| d.clone())
            .collect();

        if remaining.len() != inputs.len() {
            if remaining.is_empty() {
                // Every input was the identity: gate collapses to a constant.
                // For monotone gates the identity IS the output; for negated
                // gates (NAND/NOR) the output is the inverse.
                let result = match op {
                    BooleanOp::And | BooleanOp::Or => identity,
                    BooleanOp::Nand | BooleanOp::Nor => match identity {
                        Logic::True => Logic::False,
                        Logic::False => Logic::True,
                        _ => return Ok(false),
                    },
                    _ => return Ok(false),
                };
                let constant = create(
                    cell_type.new_like(if result == Logic::True {
                        CellType::VCC
                    } else {
                        CellType::GND
                    }),
                    inst_name + "_const".into(),
                );
                debug!(
                    "BooleanSimplify: all-identity inputs collapsed {}!",
                    output.as_net()
                );
                replace(output, constant.get_output(0))?;
                return Ok(true);
            }

            if remaining.len() == 1 {
                let single = remaining.into_iter().next().unwrap();
                match op {
                    // Monotone gates with one real input: the gate is a wire.
                    BooleanOp::And | BooleanOp::Or => {
                        debug!(
                            "BooleanSimplify: identity reduced to wire on {}!",
                            output.as_net()
                        );
                        replace(output, single)?;
                    }
                    // Negated-monotone gates with one real input: NAND(A,1)=NOT(A),
                    // NOR(A,0)=NOT(A).
                    BooleanOp::Nand | BooleanOp::Nor => {
                        let inv =
                            create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
                        inv.get_input(0).connect(single);
                        debug!(
                            "BooleanSimplify: identity reduced to INV on {}!",
                            output.as_net()
                        );
                        replace(output, inv.get_output(0))?;
                    }
                    // XOR(A,0)=A, XNOR(A,1)=A.
                    BooleanOp::Xor | BooleanOp::Xnor => {
                        debug!(
                            "BooleanSimplify: XOR/XNOR identity reduced to wire on {}!",
                            output.as_net()
                        );
                        replace(output, single)?;
                    }
                }
                return Ok(true);
            }

            // Multiple non-identity inputs remain: shrink the gate.
            let Some(new_type) = resized_boolean_type(ct, remaining.len()) else {
                return Ok(false);
            };
            let gate = create(cell_type.new_like(new_type), inst_name + "_reduced".into());
            for (idx, driver) in remaining.into_iter().enumerate() {
                gate.get_input(idx).connect(driver);
            }
            debug!(
                "BooleanSimplify: identity stripped, gate resized on {}!",
                output.as_net()
            );
            replace(output, gate.get_output(0))?;
            return Ok(true);
        }

        //  Equal inputs (idempotency): binary gates only
        if inputs.len() == 2 && inputs[0] == inputs[1] {
            match op {
                // AND(A,A)=A, OR(A,A)=A
                BooleanOp::And | BooleanOp::Or => {
                    replace(output.clone(), inputs[0].clone())?;
                }
                // NAND(A,A)=NOT(A), NOR(A,A)=NOT(A)
                BooleanOp::Nand | BooleanOp::Nor => {
                    let inv = create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
                    inv.get_input(0).connect(inputs[0].clone());
                    replace(output.clone(), inv.get_output(0))?;
                }
                // XOR(A,A)=0
                BooleanOp::Xor => {
                    let c = create(
                        cell_type.new_like(CellType::GND),
                        inst_name + "_const0".into(),
                    );
                    replace(output.clone(), c.get_output(0))?;
                }
                // XNOR(A,A)=1
                BooleanOp::Xnor => {
                    let c = create(
                        cell_type.new_like(CellType::VCC),
                        inst_name + "_const1".into(),
                    );
                    replace(output.clone(), c.get_output(0))?;
                }
            }
            debug!(
                "BooleanSimplify: equal-input simplification on {}!",
                output.as_net()
            );
            return Ok(true);
        }

        Ok(false)
    }
}

// Pattern: ComplementaryInputs

/// Simplifies two-input Boolean gates whose inputs are mutual complements.
///
/// ```text
/// AND (A, NOT(A))  = 0      NAND(A, NOT(A)) = 1
/// OR  (A, NOT(A))  = 1      NOR (A, NOT(A)) = 0
/// XOR (A, NOT(A))  = 1      XNOR(A, NOT(A)) = 0
/// ```
#[derive(Debug)]
pub struct ComplementaryInputs;

impl fmt::Display for ComplementaryInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Boolean(A, NOT(A)) simplification")
    }
}

impl Pattern for ComplementaryInputs {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();
        let Some(op) = BooleanOp::from_type(ct) else {
            return Ok(false);
        };

        // Only defined for binary cells; multi-input complementary checks
        // would require a more expensive pairwise scan — leave for later.
        if cell.inputs().count() != 2 {
            return Ok(false);
        }

        let Some(a) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b) = cell.get_input(1).get_driver() else {
            return Ok(false);
        };

        if !is_inverted_of(&a, &b) && !is_inverted_of(&b, &a) {
            return Ok(false);
        }

        let result = match op {
            BooleanOp::And | BooleanOp::Nor | BooleanOp::Xnor => Logic::False,
            BooleanOp::Or | BooleanOp::Nand | BooleanOp::Xor => Logic::True,
        };
        let constant = create(
            cell_type.new_like(if result == Logic::True {
                CellType::VCC
            } else {
                CellType::GND
            }),
            cell.get_instance_name().unwrap() + "_const".into(),
        );
        let output = cell.get_output(0);
        debug!("ComplementaryInputs: A op NOT(A) on {}!", output.as_net());
        replace(output, constant.get_output(0))?;
        Ok(true)
    }
}

// Pattern: DoubleNegation

/// NOT(NOT(A)) = A, INV(INV(A)) = A
#[derive(Debug)]
pub struct DoubleNegation;

impl fmt::Display for DoubleNegation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOT(NOT(A)) = A")
    }
}

impl Pattern for DoubleNegation {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(driver) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };

        if !matches!(
            driver.get_ptype(),
            Some(CellType::NOT) | Some(CellType::INV)
        ) {
            return Ok(false);
        }

        let Some(inner_input) = driver.unwrap().get_input(0).get_driver() else {
            return Ok(false);
        };

        let output = cell.get_output(0);
        debug!("DoubleNegation applied to cell {}!", output.as_net());
        replace(output, inner_input)?;
        Ok(true)
    }
}

// Pattern: MonotoneFold

/// Folds homogeneous AND or OR trees into a single wider gate.
///
/// ```text
/// AND2(AND2(a,b), c)  =>  AND3(a,b,c)
/// OR2(OR2(a,b), c)    =>  OR3(a,b,c)
/// ```
///
/// Fold is limited to a maximum arity of 4 (the widest available gate).
#[derive(Debug)]
pub struct MonotoneFold;

impl fmt::Display for MonotoneFold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AND2(AND2(a,b), c) => AND3(a,b,c)")
    }
}

impl MonotoneFold {
    /// Checks whether the set of child gates (all of type `op`) can be folded
    /// together with `extra` non-child inputs into a single gate of type `op`.
    fn can_combine(op: BooleanOp, children: &[CellType], extra: usize) -> Option<CellType> {
        let mut fanin = extra;
        for child in children {
            if BooleanOp::from_type(*child) != Some(op) {
                return None;
            }
            fanin += child.get_num_inputs();
        }
        boolean_type(op, fanin)
    }
}

impl Pattern for MonotoneFold {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_type = cell_type.get_type();
        let Some(root_op) = BooleanOp::from_type(root_type) else {
            return Ok(false);
        };
        if !root_op.is_monotone() {
            return Ok(false);
        }

        let mut child_drivers: Vec<NetRef<Self::I>> = Vec::new();
        let mut child_types: Vec<CellType> = Vec::new();
        let mut non_child_drivers: Vec<DrivenNet<Self::I>> = Vec::new();

        for input in cell.inputs() {
            let Some(driver) = input.get_driver() else {
                return Ok(false);
            };
            if driver.get_ptype().and_then(BooleanOp::from_type) == Some(root_op) {
                child_types.push(driver.get_ptype().unwrap());
                child_drivers.push(driver.unwrap());
            } else {
                non_child_drivers.push(driver);
            }
        }

        if child_drivers.is_empty() {
            return Ok(false);
        }

        let Some(new_type) = Self::can_combine(root_op, &child_types, non_child_drivers.len())
        else {
            return Ok(false);
        };

        let new_gate = create(
            cell_type.new_like(new_type),
            cell.get_instance_name().unwrap() + "_folded".into(),
        );

        let mut port_idx = 0;
        for child_ref in &child_drivers {
            for input in child_ref.inputs() {
                if let Some(grandchild) = input.get_driver() {
                    new_gate.get_input(port_idx).connect(grandchild);
                }
                port_idx += 1;
            }
        }
        for driver in non_child_drivers {
            new_gate.get_input(port_idx).connect(driver);
            port_idx += 1;
        }

        let old_output = cell.get_output(0);
        debug!("MonotoneFold applied to cell {}!", old_output.as_net());
        replace(old_output, new_gate.get_output(0))?;
        Ok(true)
    }
}

// Pattern: NotBooleanMap  (replaces NotAndMap + NotOrMap)

/// Merges an inverter into its two-input AND or OR driver:
///
/// ```text
/// NOT(AND(A,B))  =>  NAND(A,B)
/// NOT(AND2(A,B)) =>  NAND2(A,B)
/// NOT(OR(A,B))   =>  NOR(A,B)
/// NOT(OR2(A,B))  =>  NOR2(A,B)
/// ```
#[derive(Debug)]
pub struct NotBooleanMap;

impl fmt::Display for NotBooleanMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOT(AND/OR(A,B)) => NAND/NOR(A,B)")
    }
}

impl Pattern for NotBooleanMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::NOT | CellType::INV) {
            return Ok(false);
        }

        let Some(driver) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(source_type) = driver.get_ptype() else {
            return Ok(false);
        };
        let Some(target_type) = negated_map_type(source_type) else {
            return Ok(false);
        };

        let source = driver.unwrap();
        let Some(a) = source.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b) = source.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();
        let target = create(cell_type.new_like(target_type), inst_name + "_map".into());
        target.get_input(0).connect(a);
        target.get_input(1).connect(b);

        let output = cell.get_output(0);
        debug!(
            "NotBooleanMap: NOT(AND/OR) => NAND/NOR on {}!",
            output.as_net()
        );
        replace(output, target.get_output(0))?;
        Ok(true)
    }
}

// AOI / OAI compound mappings
//
// Shared infrastructure for all four mapping patterns.  The two dimensions
// that vary are:
//   root gate  : NOR  (for AOI family) | NAND (for OAI family)
//   child gate : AND  (for AOI family) | OR   (for OAI family)

/// Returns the compound-gate CellType for a given `(root, child)` pair, where
/// one child is of `child_type` and the remaining input is a bare signal.
///
/// ```text
/// NOR(AND(B1,B2), A) → AOI21
/// NAND(OR(B1,B2), A) → OAI21
/// ```
fn compound_type_21(root: CellType, child: CellType) -> Option<CellType> {
    match (root, child) {
        (CellType::NOR | CellType::NOR2, CellType::AND | CellType::AND2) => Some(CellType::AOI21),
        (CellType::NAND | CellType::NAND2, CellType::OR | CellType::OR2) => Some(CellType::OAI21),
        _ => None,
    }
}

/// Returns the compound-gate CellType for a given `(root, child_a, child_b)`
/// triple, where both children expand into a wider compound gate.
///
/// ```text
/// NOR(AND(A1,A2), AND(B1,B2)) → AOI22
/// NAND(OR(A1,A2), OR(B1,B2))  → OAI22
/// ```
fn compound_type_22(root: CellType, child_a: CellType, child_b: CellType) -> Option<CellType> {
    match (root, child_a, child_b) {
        (
            CellType::NOR | CellType::NOR2,
            CellType::AND | CellType::AND2,
            CellType::AND | CellType::AND2,
        ) => Some(CellType::AOI22),
        (
            CellType::NAND | CellType::NAND2,
            CellType::OR | CellType::OR2,
            CellType::OR | CellType::OR2,
        ) => Some(CellType::OAI22),
        _ => None,
    }
}

/// Returns `true` if `ct` is an AND or AND2 (usable as an AOI child).
fn is_and2(ct: CellType) -> bool {
    matches!(ct, CellType::AND | CellType::AND2)
}

/// Returns `true` if `ct` is an OR or OR2 (usable as an OAI child).
fn is_or2(ct: CellType) -> bool {
    matches!(ct, CellType::OR | CellType::OR2)
}

/// NOR(AND(B1,B2), A) = AOI21(A,B1,B2)
#[derive(Debug)]
pub struct AoiMap;

impl fmt::Display for AoiMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOR(AND(B1,B2),A) => AOI21(A,B1,B2)")
    }
}

impl Pattern for AoiMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_ct = cell_type.get_type();
        if !matches!(root_ct, CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();
        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };
        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let (and_driver, other_driver) = match (d0.get_ptype(), d1.get_ptype()) {
            (Some(t), _) if is_and2(t) => (d0, d1),
            (_, Some(t)) if is_and2(t) => (d1, d0),
            _ => return Ok(false),
        };

        let child_ct = and_driver.get_ptype().unwrap();
        let Some(target_ct) = compound_type_21(root_ct, child_ct) else {
            return Ok(false);
        };

        let and_ref = and_driver.unwrap();
        let Some(b1) = and_ref.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b2) = and_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();
        let aoi = create(cell_type.new_like(target_ct), inst_name + "_aoi21".into());
        aoi.get_input(0).connect(other_driver);
        aoi.get_input(1).connect(b1);
        aoi.get_input(2).connect(b2);

        let output = cell.get_output(0);
        debug!("AoiMap: NOR(AND(B1,B2),A) => AOI21 on {}!", output.as_net());
        replace(output, aoi.get_output(0))?;
        Ok(true)
    }
}

/// NAND(OR(B1,B2), A) = OAI21(A,B1,B2)
#[derive(Debug)]
pub struct OaiMap;

impl fmt::Display for OaiMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NAND(OR(B1,B2),A) => OAI21(A,B1,B2)")
    }
}

impl Pattern for OaiMap {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_ct = cell_type.get_type();
        if !matches!(root_ct, CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();
        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };
        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let (or_driver, other_driver) = match (d0.get_ptype(), d1.get_ptype()) {
            (Some(t), _) if is_or2(t) => (d0, d1),
            (_, Some(t)) if is_or2(t) => (d1, d0),
            _ => return Ok(false),
        };

        let child_ct = or_driver.get_ptype().unwrap();
        let Some(target_ct) = compound_type_21(root_ct, child_ct) else {
            return Ok(false);
        };

        let or_ref = or_driver.unwrap();
        let Some(b1) = or_ref.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b2) = or_ref.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();
        let oai = create(cell_type.new_like(target_ct), inst_name + "_oai21".into());
        oai.get_input(0).connect(other_driver);
        oai.get_input(1).connect(b1);
        oai.get_input(2).connect(b2);

        let output = cell.get_output(0);
        debug!("OaiMap: NAND(OR(B1,B2),A) => OAI21 on {}!", output.as_net());
        replace(output, oai.get_output(0))?;
        Ok(true)
    }
}

/// NOR(AND(A1,A2), AND(B1,B2)) = AOI22(A1,A2,B1,B2)
#[derive(Debug)]
pub struct AoiMap22;

impl fmt::Display for AoiMap22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NOR(AND(A1,A2),AND(B1,B2)) => AOI22(A1,A2,B1,B2)")
    }
}

impl Pattern for AoiMap22 {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_ct = cell_type.get_type();
        if !matches!(root_ct, CellType::NOR | CellType::NOR2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();
        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };
        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let (t0, t1) = match (d0.get_ptype(), d1.get_ptype()) {
            (Some(t0), Some(t1)) => (t0, t1),
            _ => return Ok(false),
        };

        let Some(target_ct) = compound_type_22(root_ct, t0, t1) else {
            return Ok(false);
        };

        let r0 = d0.unwrap();
        let r1 = d1.unwrap();

        let Some(a1) = r0.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(a2) = r0.get_input(1).get_driver() else {
            return Ok(false);
        };
        let Some(b1) = r1.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b2) = r1.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();
        let aoi = create(cell_type.new_like(target_ct), inst_name + "_aoi22".into());
        aoi.get_input(0).connect(a1);
        aoi.get_input(1).connect(a2);
        aoi.get_input(2).connect(b1);
        aoi.get_input(3).connect(b2);

        let output = cell.get_output(0);
        debug!("AoiMap22: NOR(AND,AND) => AOI22 on {}!", output.as_net());
        replace(output, aoi.get_output(0))?;
        Ok(true)
    }
}

/// NAND(OR(A1,A2), OR(B1,B2)) = OAI22(A1,A2,B1,B2)
#[derive(Debug)]
pub struct OaiMap22;

impl fmt::Display for OaiMap22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NAND(OR(A1,A2),OR(B1,B2)) => OAI22(A1,A2,B1,B2)")
    }
}

impl Pattern for OaiMap22 {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let root_ct = cell_type.get_type();
        if !matches!(root_ct, CellType::NAND | CellType::NAND2) {
            return Ok(false);
        }

        let inputs: Vec<_> = cell.inputs().collect();
        if inputs.len() != 2 {
            return Ok(false);
        }

        let Some(d0) = inputs[0].get_driver() else {
            return Ok(false);
        };
        let Some(d1) = inputs[1].get_driver() else {
            return Ok(false);
        };

        let (t0, t1) = match (d0.get_ptype(), d1.get_ptype()) {
            (Some(t0), Some(t1)) => (t0, t1),
            _ => return Ok(false),
        };

        let Some(target_ct) = compound_type_22(root_ct, t0, t1) else {
            return Ok(false);
        };

        let r0 = d0.unwrap();
        let r1 = d1.unwrap();

        let Some(a1) = r0.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(a2) = r0.get_input(1).get_driver() else {
            return Ok(false);
        };
        let Some(b1) = r1.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(b2) = r1.get_input(1).get_driver() else {
            return Ok(false);
        };

        let inst_name = cell.get_instance_name().unwrap();
        let oai = create(cell_type.new_like(target_ct), inst_name + "_oai22".into());
        oai.get_input(0).connect(a1);
        oai.get_input(1).connect(a2);
        oai.get_input(2).connect(b1);
        oai.get_input(3).connect(b2);

        let output = cell.get_output(0);
        debug!("OaiMap22: NAND(OR,OR) => OAI22 on {}!", output.as_net());
        replace(output, oai.get_output(0))?;
        Ok(true)
    }
}

// Pattern: MuxSameInput

/// MUX(S, A, A) = A  (select is irrelevant when both data inputs are equal)
#[derive(Debug)]
pub struct MuxSameInput;

impl fmt::Display for MuxSameInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MUX(S,A,A) = A")
    }
}

impl Pattern for MuxSameInput {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        _create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        if !matches!(cell_type.get_type(), CellType::MUX | CellType::MUX2) {
            return Ok(false);
        }

        let Some(d1) = cell.get_input(1).get_driver() else {
            return Ok(false);
        };
        let Some(d2) = cell.get_input(2).get_driver() else {
            return Ok(false);
        };

        if d1 != d2 {
            return Ok(false);
        }

        let output = cell.get_output(0);
        debug!("MuxSameInput applied to cell {}!", output.as_net());
        replace(output, d1)?;
        Ok(true)
    }
}

// Pattern: MuxConstSelect

/// Eliminates a MUX whose select line is a constant, selecting the appropriate
/// data input statically.
///
/// Also handles constant data inputs:
/// ```text
/// MUX(S, 0, 1)  =  NOT(S)      MUX2(S, 0, 1)  =  S
/// MUX(S, 1, 0)  =  S           MUX2(S, 1, 0)  =  NOT(S)
/// ```
///
/// Port conventions (indices):
/// - MUX:  port 0 = S, port 1 = A (selected when S=1), port 2 = B (selected when S=0)
/// - MUX2: port 0 = S, port 1 = B (selected when S=0), port 2 = A (selected when S=1)
#[derive(Debug)]
pub struct MuxConstSelect;

impl fmt::Display for MuxConstSelect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MUX(const S,A,B) = A or B; MUX(S,0,1)/MUX(S,1,0) = S or NOT(S)"
        )
    }
}

impl Pattern for MuxConstSelect {
    type I = Cell;

    fn apply(
        &self,
        cell: &NetRef<Self::I>,
        cell_type: &Self::I,
        create: &Create<Self::I>,
        replace: &mut Replace<Self::I>,
    ) -> Result<bool, Error> {
        let ct = cell_type.get_type();

        // (a_idx, b_idx): which port is selected when S=1 vs S=0.
        let (a_idx, b_idx) = match ct {
            CellType::MUX => (1, 2),
            CellType::MUX2 => (2, 1),
            _ => return Ok(false),
        };

        let Some(s) = cell.get_input(0).get_driver() else {
            return Ok(false);
        };
        let Some(da) = cell.get_input(a_idx).get_driver() else {
            return Ok(false);
        };
        let Some(db) = cell.get_input(b_idx).get_driver() else {
            return Ok(false);
        };

        let s_const = s.get_instance_type().and_then(|t| t.get_constant());
        let a_const = da.get_instance_type().and_then(|t| t.get_constant());
        let b_const = db.get_instance_type().and_then(|t| t.get_constant());

        let output = cell.get_output(0);
        let inst_name = cell.get_instance_name().unwrap();

        // Constant select
        if let Some(sel) = s_const {
            let chosen = match sel {
                Logic::True => da,
                Logic::False => db,
                _ => return Ok(false),
            };
            debug!(
                "MuxConstSelect: constant-select MUX resolved on {}!",
                output.as_net()
            );
            replace(output, chosen)?;
            return Ok(true);
        }

        // Constant data: MUX(S,0,1) / MUX(S,1,0)
        match (a_const, b_const) {
            // MUX(S, 1, 0) = S  (a_idx selected when S=1, holds 1; b_idx holds 0)
            (Some(Logic::True), Some(Logic::False)) => {
                debug!("MuxConstSelect: MUX(S,1,0) => S on {}!", output.as_net());
                replace(output, s)?;
                return Ok(true);
            }
            // MUX(S, 0, 1) = NOT(S)
            (Some(Logic::False), Some(Logic::True)) => {
                let inv = create(cell_type.new_like(CellType::INV), inst_name + "_inv".into());
                inv.get_input(0).connect(s);
                debug!(
                    "MuxConstSelect: MUX(S,0,1) => NOT(S) on {}!",
                    output.as_net()
                );
                replace(output, inv.get_output(0))?;
                return Ok(true);
            }
            _ => {}
        }

        Ok(false)
    }
}
