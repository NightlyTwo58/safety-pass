use safety_net::{DrivenNet, Instantiable, Net, Netlist};
use safety_pass::patterns::{
    AoiMap, AoiMap22, BooleanSimplify, ComplementaryInputs, DoubleNegation, MonotoneFold,
    MuxConstSelect, MuxSameInput, NotBooleanMap, OaiMap, OaiMap22,
};
use safety_pass::{Cell, CellType, Folder, Pass, Primitive};
use std::rc::Rc;

// Gate constructors

fn and_gate() -> Cell {
    Cell::new(CellType::AND2, None)
}
fn or_gate() -> Cell {
    Cell::new(CellType::OR2, None)
}
fn inv_gate() -> Cell {
    Cell::new(CellType::INV, None)
}
fn not_gate() -> Cell {
    Cell::new(CellType::NOT, None)
}

fn nand2_gate() -> Cell {
    Cell::new(CellType::NAND2, None)
}
fn nor2_gate() -> Cell {
    Cell::new(CellType::NOR2, None)
}
fn xor2_gate() -> Cell {
    Cell::new(CellType::XOR2, None)
}
fn xnor2_gate() -> Cell {
    Cell::new(CellType::XNOR2, None)
}

fn and3_gate() -> Cell {
    Cell::new(CellType::AND3, None)
}
fn or3_gate() -> Cell {
    Cell::new(CellType::OR3, None)
}
fn nand3_gate() -> Cell {
    Cell::new(CellType::NAND3, None)
}
fn nor3_gate() -> Cell {
    Cell::new(CellType::NOR3, None)
}

fn and4_gate() -> Cell {
    Cell::new(CellType::AND4, None)
}
fn or4_gate() -> Cell {
    Cell::new(CellType::OR4, None)
}

fn vcc(nl: &Rc<Netlist<Cell>>) -> DrivenNet<Cell> {
    nl.insert_constant(safety_net::Logic::True, "vcc".into())
        .unwrap()
}

fn gnd(nl: &Rc<Netlist<Cell>>) -> DrivenNet<Cell> {
    nl.insert_constant(safety_net::Logic::False, "gnd".into())
        .unwrap()
}

// Shared netlist fixture (idempotent AND(g,g) pattern)

fn ex_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┬── AND2(inst_1) ── y
    //                  └── (same wire)
    // b ─┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g.clone(), g])
        .unwrap();
    h.expose_with_name("y".into());
    nl
}

// Folder / convergence mechanics

#[test]
fn test_ld_pattern() {
    // Sanity: BooleanSimplify fires exactly once on AND(g,g) = g.
    let nl = ex_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);

    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 1, before);
    assert!(res.unwrap().contains("1 iterations"));
}

#[test]
fn test_run_twice_pattern() {
    // Running the folder a second time on an already-converged netlist should
    // report 0 iterations and make no further changes.
    let nl = ex_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);

    let before = nl.len();
    let res1 = folder.run(&nl);
    assert!(res1.is_ok());
    assert_eq!(nl.len() + 1, before);

    let after = nl.len();
    let res2 = folder.run(&nl);
    assert!(res2.is_ok());
    assert_eq!(nl.len(), after);
    assert!(res2.unwrap().contains("0 iterations"));
}

// MonotoneFold netlists and tests

fn monotone_and_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┐
    // b ─┘             ├── AND2(inst_1) ── y
    // c ───────────────┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g, c])
        .unwrap();
    h.expose_with_name("y".into());
    nl
}

fn monotone_and4_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┐
    // b ─┘             ├── AND2(inst_2) ── y
    // c ─┐             │
    //    AND2(inst_1) ─┘
    // d ─┘
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let d = nl.insert_input(Net::new_logic("d".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(and_gate(), "inst_1".into(), &[c, d])
        .unwrap()
        .get_output(0);
    let top = nl
        .insert_gate(and_gate(), "inst_2".into(), &[g, h])
        .unwrap();
    top.expose_with_name("y".into());
    nl
}

fn monotone_or_netlist() -> Rc<Netlist<Cell>> {
    // Same shape as monotone_and_netlist but with OR gates.
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl.insert_gate(or_gate(), "inst_1".into(), &[g, c]).unwrap();
    h.expose_with_name("y".into());
    nl
}

fn monotone_or4_netlist() -> Rc<Netlist<Cell>> {
    // OR2(OR2(a,b), OR2(c,d)) => OR4(a,b,c,d)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let d = nl.insert_input(Net::new_logic("d".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(or_gate(), "inst_1".into(), &[c, d])
        .unwrap()
        .get_output(0);
    let top = nl.insert_gate(or_gate(), "inst_2".into(), &[g, h]).unwrap();
    top.expose_with_name("y".into());
    nl
}

fn monotone_no_fold_netlist() -> Rc<Netlist<Cell>> {
    // inst_0 output is shared between two AND parents, so only one of them can
    // absorb it; the other must remain.
    //
    // a ─┐
    //    AND2(inst_0) ─┬── AND2(inst_1) ── y1
    // b ─┘             └── AND2(inst_2) ── y2
    // c ──────────────────────────────────┘ (into inst_1)
    // d ──────────────────────────────────┘ (into inst_2)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let d = nl.insert_input(Net::new_logic("d".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h1 = nl
        .insert_gate(and_gate(), "inst_1".into(), &[g.clone(), c])
        .unwrap();
    let h2 = nl
        .insert_gate(and_gate(), "inst_2".into(), &[g, d])
        .unwrap();
    h1.expose_with_name("y1".into());
    h2.expose_with_name("y2".into());
    nl
}

#[test]
fn test_monotone_fold_and3() {
    // AND2(AND2(a,b), c) => AND3(a,b,c)
    // before: 5 objects (a, b, c, inst_0, inst_1)
    // after:  4 objects (a, b, c, inst_1_folded)
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 1, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 4);
    assert_eq!(gates[3].get_ptype(), Some(CellType::AND3));
}

#[test]
fn test_monotone_fold_and4() {
    // AND2(AND2(a,b), AND2(c,d)) => AND4(a,b,c,d)
    // before: 7 objects (a, b, c, d, inst_0, inst_1, inst_2)
    // after:  5 objects (a, b, c, d, inst_2_folded)
    let nl = monotone_and4_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 2, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 5);
    assert_eq!(gates[4].get_ptype(), Some(CellType::AND4));
}

#[test]
fn test_monotone_fold_or3() {
    // OR2(OR2(a,b), c) => OR3(a,b,c)
    let nl = monotone_or_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 1, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 4);
    assert_eq!(gates[3].get_ptype(), Some(CellType::OR3));
}

#[test]
fn test_monotone_fold_or4() {
    // OR2(OR2(a,b), OR2(c,d)) => OR4(a,b,c,d)
    let nl = monotone_or4_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 2, before);
    let gates: Vec<_> = nl.objects().collect();
    assert_eq!(gates.len(), 5);
    assert_eq!(gates[4].get_ptype(), Some(CellType::OR4));
}

#[test]
fn test_monotone_fold_idempotent_after() {
    // After folding to AND3, a second folder run should make no changes.
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let res1 = folder.run(&nl);
    assert!(res1.is_ok());
    let after_first = nl.len();
    let res2 = folder.run(&nl);
    assert!(res2.is_ok());
    assert_eq!(nl.len(), after_first);
    assert!(res2.unwrap().contains("0 iterations"));
}

#[test]
fn test_monotone_no_fold_shared_child() {
    // inst_0 is shared; only one of its two parents can absorb it per pass.
    // The folder converges in 2 iterations (one fold per pass).
    let nl = monotone_no_fold_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    assert!(res.unwrap().contains("2 iterations"));
}

// BooleanSimplify — constant folding (binary gates)

fn and_const0_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn and_const1_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or_const1_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or_const0_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand_const0_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nand2_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand_const1_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nand2_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor_const1_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nor2_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor_const0_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nor2_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_constant_fold_and0() {
    // AND(a, 0) = 0  — absorbing element
    // before: 3 objects (a, gnd, inst_0)
    // after:  2 objects (a, gnd), output rewired to existing GND
    let nl = and_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_constant_fold_and1() {
    // AND(a, 1) = a  — identity element
    // before: 3 objects (a, vcc, inst_0)
    // after:  1 object (a), gate and vcc cleaned
    let nl = and_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_constant_fold_or1() {
    // OR(a, 1) = 1  — absorbing element
    let nl = or_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_constant_fold_or0() {
    // OR(a, 0) = a  — identity element
    let nl = or_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_constant_fold_nand0() {
    // NAND(a, 0) = 1  — absorbing element (inverted)
    let nl = nand_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_constant_fold_nand1() {
    // NAND(a, 1) = NOT(a)  — identity element (inverted)
    let nl = nand_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

#[test]
fn test_constant_fold_nor1() {
    // NOR(a, 1) = 0  — absorbing element (inverted)
    let nl = nor_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_constant_fold_nor0() {
    // NOR(a, 0) = NOT(a)  — identity element (inverted)
    let nl = nor_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
}

// BooleanSimplify — constant folding (3-input gates, new coverage)

fn and3_absorb_netlist() -> Rc<Netlist<Cell>> {
    // AND3(a, b, 0) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(and3_gate(), "inst_0".into(), &[a, b, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn and3_identity_netlist() -> Rc<Netlist<Cell>> {
    // AND3(a, b, 1) = AND2(a, b)  — shrinks the gate
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(and3_gate(), "inst_0".into(), &[a, b, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or3_absorb_netlist() -> Rc<Netlist<Cell>> {
    // OR3(a, b, 1) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(or3_gate(), "inst_0".into(), &[a, b, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or3_identity_netlist() -> Rc<Netlist<Cell>> {
    // OR3(a, b, 0) = OR2(a, b)  — shrinks the gate
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(or3_gate(), "inst_0".into(), &[a, b, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand3_absorb_netlist() -> Rc<Netlist<Cell>> {
    // NAND3(a, b, 0) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(nand3_gate(), "inst_0".into(), &[a, b, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand3_identity_netlist() -> Rc<Netlist<Cell>> {
    // NAND3(a, b, 1) = NAND2(a, b)  — shrinks the gate
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(nand3_gate(), "inst_0".into(), &[a, b, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor3_absorb_netlist() -> Rc<Netlist<Cell>> {
    // NOR3(a, b, 1) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(nor3_gate(), "inst_0".into(), &[a, b, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor3_identity_netlist() -> Rc<Netlist<Cell>> {
    // NOR3(a, b, 0) = NOR2(a, b)  — shrinks the gate
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(nor3_gate(), "inst_0".into(), &[a, b, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn and4_all_const_netlist() -> Rc<Netlist<Cell>> {
    // AND4(1, 1, 1, 1) = 1  — full constant fold on wide gate
    let nl = Netlist::new("top".into());
    let v = vcc(&nl);
    let g = nl
        .insert_gate(
            and4_gate(),
            "inst_0".into(),
            &[v.clone(), v.clone(), v.clone(), v],
        )
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or4_all_const_netlist() -> Rc<Netlist<Cell>> {
    // OR4(0, 0, 0, 0) = 0
    let nl = Netlist::new("top".into());
    let z = gnd(&nl);
    let g = nl
        .insert_gate(
            or4_gate(),
            "inst_0".into(),
            &[z.clone(), z.clone(), z.clone(), z],
        )
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_and3_absorb() {
    // AND3(a, b, 0) = 0
    let nl = and3_absorb_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_and3_identity_shrinks_gate() {
    // AND3(a, b, 1) => AND2(a, b)
    let nl = and3_identity_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::AND2));
}

#[test]
fn test_or3_absorb() {
    // OR3(a, b, 1) = 1
    let nl = or3_absorb_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_or3_identity_shrinks_gate() {
    // OR3(a, b, 0) => OR2(a, b)
    let nl = or3_identity_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::OR2));
}

#[test]
fn test_nand3_absorb() {
    // NAND3(a, b, 0) = 1
    let nl = nand3_absorb_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_nand3_identity_shrinks_gate() {
    // NAND3(a, b, 1) => NAND2(a, b)
    let nl = nand3_identity_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::NAND2));
}

#[test]
fn test_nor3_absorb() {
    // NOR3(a, b, 1) = 0
    let nl = nor3_absorb_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_nor3_identity_shrinks_gate() {
    // NOR3(a, b, 0) => NOR2(a, b)
    let nl = nor3_identity_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::NOR2));
}

#[test]
fn test_and4_all_const() {
    // AND4(1,1,1,1) = 1
    let nl = and4_all_const_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_or4_all_const() {
    // OR4(0,0,0,0) = 0
    let nl = or4_all_const_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

// BooleanSimplify — XOR / XNOR constant and identity

fn xor_const0_netlist() -> Rc<Netlist<Cell>> {
    // XOR(a, 0) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xor_const1_netlist() -> Rc<Netlist<Cell>> {
    // XOR(a, 1) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_const1_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(a, 1) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[a, vcc(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_const0_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(a, 0) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[a, gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xor_both_const_netlist() -> Rc<Netlist<Cell>> {
    // XOR(1, 0) = 1
    let nl = Netlist::new("top".into());
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[vcc(&nl), gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xor_same_input_netlist() -> Rc<Netlist<Cell>> {
    // XOR(a, a) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_same_input_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(a, a) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_xor_identity() {
    // XOR(a, 0) = a
    let nl = xor_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_xor_const1_becomes_inv() {
    // XOR(a, 1) = NOT(a)
    let nl = xor_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    // The INV should feed from input a
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

#[test]
fn test_xnor_identity() {
    // XNOR(a, 1) = a
    let nl = xnor_const1_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_xnor_const0_becomes_inv() {
    // XNOR(a, 0) = NOtest_mux_const1_selectT(a)
    let nl = xnor_const0_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
}

#[test]
fn test_xor_both_const() {
    // XOR(1, 0) = 1
    let nl = xor_both_const_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_xor_same_input() {
    // XOR(a, a) = 0
    let nl = xor_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_xnor_same_input() {
    // XNOR(a, a) = 1
    let nl = xnor_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

// BooleanSimplify — equal-input (idempotent) for AND/OR/NAND/NOR

fn and_same_input_netlist() -> Rc<Netlist<Cell>> {
    // AND(a, a) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn or_same_input_netlist() -> Rc<Netlist<Cell>> {
    // OR(a, a) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nand_same_input_netlist() -> Rc<Netlist<Cell>> {
    // NAND(a, a) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nand2_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn nor_same_input_netlist() -> Rc<Netlist<Cell>> {
    // NOR(a, a) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(nor2_gate(), "inst_0".into(), &[a.clone(), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_and_same_input() {
    // AND(a, a) = a
    let nl = and_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 1, before);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_or_same_input() {
    // OR(a, a) = a
    let nl = or_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len() + 1, before);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_nand_same_input() {
    // NAND(a, a) = NOT(a)
    let nl = nand_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
}

#[test]
fn test_nor_same_input() {
    // NOR(a, a) = NOT(a)
    let nl = nor_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
}

// DoubleNegation

fn double_neg_netlist() -> Rc<Netlist<Cell>> {
    // INV(INV(a)) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let i1 = nl
        .insert_gate(inv_gate(), "inv1".into(), &[a])
        .unwrap()
        .get_output(0);
    let i2 = nl.insert_gate(inv_gate(), "inv2".into(), &[i1]).unwrap();
    i2.expose_with_name("y".into());
    nl
}

fn double_neg_not_netlist() -> Rc<Netlist<Cell>> {
    // NOT(NOT(a)) = a  (using NOT cells)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let n1 = nl
        .insert_gate(not_gate(), "not1".into(), &[a])
        .unwrap()
        .get_output(0);
    let n2 = nl.insert_gate(not_gate(), "not2".into(), &[n1]).unwrap();
    n2.expose_with_name("y".into());
    nl
}

fn double_neg_mixed_netlist() -> Rc<Netlist<Cell>> {
    // INV(NOT(a)) = a  (mixed INV/NOT cells)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let n1 = nl
        .insert_gate(not_gate(), "not1".into(), &[a])
        .unwrap()
        .get_output(0);
    let i2 = nl.insert_gate(inv_gate(), "inv2".into(), &[n1]).unwrap();
    i2.expose_with_name("y".into());
    nl
}

fn single_inv_netlist() -> Rc<Netlist<Cell>> {
    // Single INV — should NOT fire
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let i = nl.insert_gate(inv_gate(), "inv1".into(), &[a]).unwrap();
    i.expose_with_name("y".into());
    nl
}

#[test]
fn test_double_negation_inv() {
    // INV(INV(a)) = a
    // before: 3 objects (a, inv1, inv2)
    // after:  1 object  (a)
    let nl = double_neg_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_not() {
    // NOT(NOT(a)) = a
    let nl = double_neg_not_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_mixed() {
    // INV(NOT(a)) = a  — mixed cell types should both be recognised
    let nl = double_neg_mixed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_double_negation_no_fire_single() {
    // Single INV should not be touched
    let nl = single_inv_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(DoubleNegation);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

// ComplementaryInputs  (new pattern)

fn make_inv_of(nl: &Rc<Netlist<Cell>>, a: DrivenNet<Cell>, name: &str) -> DrivenNet<Cell> {
    nl.insert_gate(inv_gate(), name.into(), &[a])
        .unwrap()
        .get_output(0)
}

fn complementary_and_netlist() -> Rc<Netlist<Cell>> {
    // AND(a, NOT(a)) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_or_netlist() -> Rc<Netlist<Cell>> {
    // OR(a, NOT(a)) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_nand_netlist() -> Rc<Netlist<Cell>> {
    // NAND(a, NOT(a)) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(nand2_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_nor_netlist() -> Rc<Netlist<Cell>> {
    // NOR(a, NOT(a)) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(nor2_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_xor_netlist() -> Rc<Netlist<Cell>> {
    // XOR(a, NOT(a)) = 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_xnor_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(a, NOT(a)) = 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[a, nota])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_reversed_netlist() -> Rc<Netlist<Cell>> {
    // AND(NOT(a), a) = 0  — inverter on the first input, plain on the second
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let nota = make_inv_of(&nl, a.clone(), "inv");
    // Reversed port order: NOT(a) on port 0, a on port 1
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[nota, a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn complementary_no_fire_netlist() -> Rc<Netlist<Cell>> {
    // AND(a, b) — independent inputs, should not fire
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let g = nl
        .insert_gate(and_gate(), "inst_0".into(), &[a, b])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

#[test]
fn test_complementary_and() {
    // AND(a, NOT(a)) = 0
    let nl = complementary_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_complementary_or() {
    // OR(a, NOT(a)) = 1
    let nl = complementary_or_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_complementary_nand() {
    // NAND(a, NOT(a)) = 1
    let nl = complementary_nand_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_complementary_nor() {
    // NOR(a, NOT(a)) = 0
    let nl = complementary_nor_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_complementary_xor() {
    // XOR(a, NOT(a)) = 1
    let nl = complementary_xor_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_complementary_xnor() {
    // XNOR(a, NOT(a)) = 0
    let nl = complementary_xnor_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_complementary_reversed_ports() {
    // AND(NOT(a), a) = 0  — inverter on port 0, plain on port 1
    let nl = complementary_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_complementary_no_fire() {
    // AND(a, b) with independent inputs — should not fire
    let nl = complementary_no_fire_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(ComplementaryInputs);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

// NotBooleanMap  (replaces NotAndMap + NotOrMap)

fn not_and_netlist() -> Rc<Netlist<Cell>> {
    // NOT(AND(a,b)) — uses unsized AND
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let and = nl
        .insert_gate(and_gate(), "and".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let not = nl.insert_gate(inv_gate(), "inv".into(), &[and]).unwrap();
    not.expose_with_name("y".into());
    nl
}

fn not_and2_netlist() -> Rc<Netlist<Cell>> {
    // NOT(AND2(a,b)) — explicit AND2
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let and = nl
        .insert_gate(and_gate(), "and2".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let not = nl.insert_gate(inv_gate(), "inv".into(), &[and]).unwrap();
    not.expose_with_name("y".into());
    nl
}

fn not_or_netlist() -> Rc<Netlist<Cell>> {
    // NOT(OR(a,b))
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let or = nl
        .insert_gate(or_gate(), "or".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let not = nl.insert_gate(inv_gate(), "inv".into(), &[or]).unwrap();
    not.expose_with_name("y".into());
    nl
}

fn not_or2_netlist() -> Rc<Netlist<Cell>> {
    // NOT(OR2(a,b)) — explicit OR2
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let or = nl
        .insert_gate(or_gate(), "or2".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let not = nl.insert_gate(inv_gate(), "inv".into(), &[or]).unwrap();
    not.expose_with_name("y".into());
    nl
}

fn not_boolean_no_fire_netlist() -> Rc<Netlist<Cell>> {
    // INV driving a NAND — should not fire (NAND is not AND/OR)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let nand = nl
        .insert_gate(nand2_gate(), "nand".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let not = nl.insert_gate(inv_gate(), "inv".into(), &[nand]).unwrap();
    not.expose_with_name("y".into());
    nl
}

#[test]
fn test_not_and_map() {
    // NOT(AND(a,b)) => NAND(a,b) — one gate fewer
    let nl = not_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotBooleanMap);
    let before = nl.len();
    assert_eq!(before, 4);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NAND2
    );
}

#[test]
fn test_not_and2_map() {
    // NOT(AND2(a,b)) => NAND2(a,b)
    let nl = not_and2_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotBooleanMap);
    let before = nl.len();
    assert_eq!(before, 4);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NAND2
    );
}

#[test]
fn test_not_or_map() {
    // NOT(OR(a,b)) => NOR(a,b)
    let nl = not_or_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotBooleanMap);
    let before = nl.len();
    assert_eq!(before, 4);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NOR2
    );
}

#[test]
fn test_not_or2_map() {
    // NOT(OR2(a,b)) => NOR2(a,b)
    let nl = not_or2_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotBooleanMap);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::NOR2
    );
}

#[test]
fn test_not_boolean_no_fire_on_nand() {
    // INV(NAND(a,b)) — NAND is already negated, pattern should not fire
    let nl = not_boolean_no_fire_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(NotBooleanMap);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

// AOI / OAI compound mappings

fn aoi21_netlist() -> Rc<Netlist<Cell>> {
    // NOR(AND(b1,b2), a) = AOI21(a,b1,b2)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));
    let and = nl
        .insert_gate(and_gate(), "and".into(), &[b1, b2])
        .unwrap()
        .get_output(0);
    let nor = nl
        .insert_gate(nor2_gate(), "nor".into(), &[and, a])
        .unwrap();
    nor.expose_with_name("y".into());
    nl
}

fn aoi21_reversed_netlist() -> Rc<Netlist<Cell>> {
    // NOR(a, AND(b1,b2)) — AND on port 1 instead of port 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));
    let and = nl
        .insert_gate(and_gate(), "and".into(), &[b1, b2])
        .unwrap()
        .get_output(0);
    let nor = nl
        .insert_gate(nor2_gate(), "nor".into(), &[a, and])
        .unwrap();
    nor.expose_with_name("y".into());
    nl
}

fn oai21_netlist() -> Rc<Netlist<Cell>> {
    // NAND(OR(b1,b2), a) = OAI21(a,b1,b2)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));
    let or = nl
        .insert_gate(or_gate(), "or".into(), &[b1, b2])
        .unwrap()
        .get_output(0);
    let nand = nl
        .insert_gate(nand2_gate(), "nand".into(), &[or, a])
        .unwrap();
    nand.expose_with_name("y".into());
    nl
}

fn aoi22_netlist() -> Rc<Netlist<Cell>> {
    // NOR(AND(a1,a2), AND(b1,b2)) = AOI22(a1,a2,b1,b2)
    let nl = Netlist::new("top".into());
    let a1 = nl.insert_input(Net::new_logic("a1".into()));
    let a2 = nl.insert_input(Net::new_logic("a2".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));
    let and1 = nl
        .insert_gate(and_gate(), "and1".into(), &[a1, a2])
        .unwrap()
        .get_output(0);
    let and2 = nl
        .insert_gate(and_gate(), "and2".into(), &[b1, b2])
        .unwrap()
        .get_output(0);
    let nor = nl
        .insert_gate(nor2_gate(), "nor".into(), &[and1, and2])
        .unwrap();
    nor.expose_with_name("y".into());
    nl
}

fn oai22_netlist() -> Rc<Netlist<Cell>> {
    // NAND(OR(a1,a2), OR(b1,b2)) = OAI22(a1,a2,b1,b2)
    let nl = Netlist::new("top".into());
    let a1 = nl.insert_input(Net::new_logic("a1".into()));
    let a2 = nl.insert_input(Net::new_logic("a2".into()));
    let b1 = nl.insert_input(Net::new_logic("b1".into()));
    let b2 = nl.insert_input(Net::new_logic("b2".into()));
    let or1 = nl
        .insert_gate(or_gate(), "or1".into(), &[a1, a2])
        .unwrap()
        .get_output(0);
    let or2 = nl
        .insert_gate(or_gate(), "or2".into(), &[b1, b2])
        .unwrap()
        .get_output(0);
    let nand = nl
        .insert_gate(nand2_gate(), "nand".into(), &[or1, or2])
        .unwrap();
    nand.expose_with_name("y".into());
    nl
}

#[test]
fn test_aoi_map() {
    // NOR(AND(b1,b2), a) => AOI21(a,b1,b2)
    let nl = aoi21_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::AOI21
    );
}

#[test]
fn test_aoi_map_reversed_ports() {
    // NOR(a, AND(b1,b2)) — AND on port 1; pattern must still fire
    let nl = aoi21_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::AOI21
    );
}

#[test]
fn test_oai_map() {
    // NAND(OR(b1,b2), a) => OAI21(a,b1,b2)
    let nl = oai21_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OaiMap);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::OAI21
    );
}

#[test]
fn test_aoi_map22() {
    // NOR(AND(a1,a2), AND(b1,b2)) => AOI22(a1,a2,b1,b2)
    let nl = aoi22_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap22);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::AOI22
    );
}

#[test]
fn test_oai_map22() {
    // NAND(OR(a1,a2), OR(b1,b2)) => OAI22(a1,a2,b1,b2)
    let nl = oai22_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(OaiMap22);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_type(),
        CellType::OAI22
    );
}

#[test]
fn test_aoi_map22_no_fire_on_21() {
    // AoiMap22 must not fire when only one child is AND — that is AoiMap's job.
    let nl = aoi21_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(AoiMap22);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

// MuxSameInput

fn mux_same_input_netlist() -> Rc<Netlist<Cell>> {
    // MUX(s, a, a) = a
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    let a = nl.insert_input(Net::new_logic("a".into()));
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[s, a.clone(), a],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux2_same_input_netlist() -> Rc<Netlist<Cell>> {
    // MUX2(s, a, a) = a  — alternate MUX cell type
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    let a = nl.insert_input(Net::new_logic("a".into()));
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX2, None),
            "mux2".into(),
            &[s, a.clone(), a],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux_diff_input_netlist() -> Rc<Netlist<Cell>> {
    // MUX(s, a, b) — different data inputs, should not fire
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let mux = nl
        .insert_gate(Cell::new(CellType::MUX, None), "mux".into(), &[s, a, b])
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

#[test]
fn test_mux_same_input() {
    // MUX(s, a, a) = a
    let nl = mux_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxSameInput);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux2_same_input() {
    // MUX2(s, a, a) = a
    let nl = mux2_same_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxSameInput);
    let before = nl.len();
    assert_eq!(before, 3);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux_same_input_no_fire() {
    // MUX(s, a, b) — different inputs, should not fire
    let nl = mux_diff_input_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxSameInput);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before);
    assert!(res.unwrap().contains("0 iterations"));
}

// MuxConstSelect

fn mux_const1_select_netlist() -> Rc<Netlist<Cell>> {
    // MUX(1, a, b) = a
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[vcc(&nl), a, b],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux_const0_select_netlist() -> Rc<Netlist<Cell>> {
    // MUX(0, a, b) = b
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[gnd(&nl), a, b],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux2_const1_select_netlist() -> Rc<Netlist<Cell>> {
    // MUX2(1, b_port, a_port) — port 2 is selected when S=1 for MUX2
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    // MUX2: port0=S, port1=B(S=0), port2=A(S=1)
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX2, None),
            "mux2".into(),
            &[vcc(&nl), b, a],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux_data_10_netlist() -> Rc<Netlist<Cell>> {
    // MUX(S, 1, 0) = S
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    // MUX: port0=S, port1=A(S=1)=1, port2=B(S=0)=0
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[s, vcc(&nl), gnd(&nl)],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

fn mux_data_01_netlist() -> Rc<Netlist<Cell>> {
    // MUX(S, 0, 1) = NOT(S)
    let nl = Netlist::new("top".into());
    let s = nl.insert_input(Net::new_logic("s".into()));
    // MUX: port0=S, port1=A(S=1)=0, port2=B(S=0)=1
    let mux = nl
        .insert_gate(
            Cell::new(CellType::MUX, None),
            "mux".into(),
            &[s, gnd(&nl), vcc(&nl)],
        )
        .unwrap();
    mux.expose_with_name("y".into());
    nl
}

#[test]
fn test_mux_const1_select() {
    // MUX(1, a, b) = a
    let nl = mux_const1_select_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);
    let before = nl.len();
    assert_eq!(before, 4);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    // MUX + VCC removed; b input orphaned and cleaned
    assert_eq!(nl.len(), before - 2);
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
    // Verify the surviving input is 'a', not 'b'
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 2);
}

#[test]
fn test_mux_const0_select() {
    // MUX(0, a, b) = b
    let nl = mux_const0_select_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux2_const_select() {
    // MUX2(1, b, a) = a  — correct port ordering for MUX2
    let nl = mux2_const1_select_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux_data_10_becomes_select() {
    // MUX(S, 1, 0) = S
    let nl = mux_data_10_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    // Output should be driven directly by input s
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_mux_data_01_becomes_not_select() {
    // MUX(S, 0, 1) = NOT(S)
    let nl = mux_data_01_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MuxConstSelect);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    // The INV should be driven by input s
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

fn xor_const1_reversed_netlist() -> Rc<Netlist<Cell>> {
    // XOR(1, a) = NOT(a) — constant on port 0 instead of port 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[vcc(&nl), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_const0_reversed_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(0, a) = NOT(a) — constant on port 0 instead of port 1
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[gnd(&nl), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xor_identity_reversed_netlist() -> Rc<Netlist<Cell>> {
    // XOR(0, a) = a — identity constant on port 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[gnd(&nl), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_identity_reversed_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(1, a) = a — identity constant on port 0
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[vcc(&nl), a])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_both_const_true_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(1, 1) = 1
    let nl = Netlist::new("top".into());
    let one = vcc(&nl);
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[one.clone(), one])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xnor_both_const_mixed_netlist() -> Rc<Netlist<Cell>> {
    // XNOR(1, 0) = 0
    let nl = Netlist::new("top".into());
    let g = nl
        .insert_gate(xnor2_gate(), "inst_0".into(), &[vcc(&nl), gnd(&nl)])
        .unwrap();
    g.expose_with_name("y".into());
    nl
}

fn xor_const1_fanout_netlist() -> Rc<Netlist<Cell>> {
    // Two independent gates sharing driver 'a':
    //   y1 = XOR(a, 1)  = NOT(a)
    //   y2 = XNOR(a, 0) = NOT(a)
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let g1 = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a.clone(), vcc(&nl)])
        .unwrap();
    let g2 = nl
        .insert_gate(xnor2_gate(), "inst_1".into(), &[a, gnd(&nl)])
        .unwrap();
    g1.expose_with_name("y1".into());
    g2.expose_with_name("y2".into());
    nl
}

fn xor_double_invert_netlist() -> Rc<Netlist<Cell>> {
    // XOR(XOR(a, 1), 1) = NOT(NOT(a)) = a
    // Exercises BooleanSimplify + DoubleNegation converging together.
    let nl = Netlist::new("top".into());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let one = vcc(&nl);
    let g1 = nl
        .insert_gate(xor2_gate(), "inst_0".into(), &[a, one.clone()])
        .unwrap()
        .get_output(0);
    let g2 = nl
        .insert_gate(xor2_gate(), "inst_1".into(), &[g1, one])
        .unwrap();
    g2.expose_with_name("y".into());
    nl
}

#[test]
fn test_xor_const1_becomes_inv_reversed() {
    // XOR(1, a) = NOT(a) — constant-first ordering must be handled too.
    let nl = xor_const1_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

#[test]
fn test_xnor_const0_becomes_inv_reversed() {
    // XNOR(0, a) = NOT(a) — constant-first ordering must be handled too.
    let nl = xnor_const0_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(driver.get_ptype(), Some(CellType::INV));
    let inv_input = driver.get_input(0).get_driver().unwrap();
    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inv_input, inputs[0]);
}

#[test]
fn test_xor_identity_reversed() {
    // XOR(0, a) = a — identity constant on port 0 (not just port 1).
    let nl = xor_identity_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_xnor_identity_reversed() {
    // XNOR(1, a) = a — identity constant on port 0 (not just port 1).
    let nl = xnor_identity_reversed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());
}

#[test]
fn test_xnor_both_const_true() {
    // XNOR(1, 1) = 1
    let nl = xnor_both_const_true_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::True)
    );
}

#[test]
fn test_xnor_both_const_mixed() {
    // XNOR(1, 0) = 0
    let nl = xnor_both_const_mixed_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    let driver = outputs[0].0.clone().unwrap();
    assert_eq!(
        driver.get_instance_type().unwrap().get_constant(),
        Some(safety_net::Logic::False)
    );
}

#[test]
fn test_xor_xnor_const_inv_shared_fanout() {
    // Two gates driven by the same input both fold to INV(a); the shared
    // driver 'a' must end up feeding both resulting inverters.
    let nl = xor_const1_fanout_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    let res = folder.run(&nl);
    assert!(res.is_ok());

    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 2);
    for (driver, _name) in outputs.iter() {
        let driver = driver.clone().unwrap();
        assert_eq!(driver.get_ptype(), Some(CellType::INV));
        let inv_input = driver.get_input(0).get_driver().unwrap();
        assert_eq!(inv_input, inputs[0]);
    }
}

#[test]
fn test_xor_double_invert_converges_to_wire() {
    // XOR(XOR(a,1),1) = NOT(NOT(a)) = a.
    // Requires BooleanSimplify (to produce the two INVs) and DoubleNegation
    // (to cancel them) to converge together across folder iterations.
    let nl = xor_double_invert_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(BooleanSimplify);
    folder.insert(DoubleNegation);
    let res = folder.run(&nl);
    assert!(res.is_ok());

    let outputs = nl.outputs();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].0.is_an_input());

    let inputs: Vec<_> = nl.inputs().collect();
    assert_eq!(inputs.len(), 1);

    let output_driver = outputs[0].0.clone().unwrap().get_output(0);
    assert_eq!(output_driver, inputs[0]);
}
