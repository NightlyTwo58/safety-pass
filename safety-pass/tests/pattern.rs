use safety_net::{Net, Netlist};
use safety_pass::{Cell, CellType, Folder, Pass, patterns::Idempotent, patterns::MonotoneFold};
use std::rc::Rc;

fn and_gate() -> Cell {
    Cell::new(CellType::AND2, None)
}

fn ex_netlist() -> Rc<Netlist<Cell>> {
    let nl = Netlist::new("top".to_string());
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

#[test]
fn test_ld_pattern() {
    let nl = ex_netlist();

    let mut folder = Folder::<Cell>::new(101);
    folder.insert(Idempotent);

    let before = nl.len();

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let after = nl.len();
    assert_eq!(after + 1, before);

    assert_eq!(res.unwrap(), "Folded 1 patterns over 1 iterations");
}

#[test]
fn test_run_twice_pattern() {
    let nl = ex_netlist();

    let mut folder = Folder::<Cell>::new(101);
    folder.insert(Idempotent);

    let before = nl.len();

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let after = nl.len();
    assert_eq!(after + 1, before);

    let res = folder.run(&nl);
    assert!(res.is_ok());

    let fin = nl.len();

    assert_eq!(fin, after);

    assert_eq!(res.unwrap(), "Folded 1 patterns over 0 iterations");
}

fn or_gate() -> Cell {
    Cell::new(CellType::OR2, None)
}

fn monotone_and_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┐
    // b ─┘             ├── AND2(inst_1) ── y
    // c ───────────────┘
    let nl = Netlist::new("top".to_string());
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
    let nl = Netlist::new("top".to_string());
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
    // Same shape as monotone_and_netlist but with OR gates
    let nl = Netlist::new("top".to_string());
    let a = nl.insert_input(Net::new_logic("a".into()));
    let b = nl.insert_input(Net::new_logic("b".into()));
    let c = nl.insert_input(Net::new_logic("c".into()));
    let g = nl
        .insert_gate(or_gate(), "inst_0".into(), &[a, b])
        .unwrap()
        .get_output(0);
    let h = nl
        .insert_gate(or_gate(), "inst_1".into(), &[g, c])
        .unwrap();
    h.expose_with_name("y".into());
    nl
}

fn monotone_no_fold_netlist() -> Rc<Netlist<Cell>> {
    // a ─┐
    //    AND2(inst_0) ─┬── AND2(inst_1) ── y1
    // b ─┘             └── AND2(inst_2) ── y2
    // c ───────────────┘
    // d ───────────────┘
    // after merge
    // a ─┬── AND3(inst_2_folded) ── y2
    // b ─┤
    // d ─┘
    let nl = Netlist::new("top".to_string());
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
    // after:  4 objects (a, b, c, inst_1_folded), inst_0 orphaned and cleaned
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 5);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let after = nl.len();
    assert_eq!(after + 1, before);
    // Verify the remaining gate is AND3
    let gates: Vec<_> = nl
        .objects()
        .filter(|n| n.get_instance_type().is_some())
        .filter(|n| !n.is_an_input())
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(
        gates[0].get_instance_type().unwrap().get_type(),
        CellType::AND3
    );
}

#[test]
fn test_monotone_fold_and4() {
    // AND2(AND2(a,b), AND2(c,d)) => AND4(a,b,c,d)
    // before: 7 objects (a, b, c, d, inst_0, inst_1, inst_2)
    // after:  5 objects (a, b, c, d, inst_2_folded), inst_0 and inst_1 cleaned
    let nl = monotone_and4_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    assert_eq!(before, 7);
    let res = folder.run(&nl);
    assert!(res.is_ok());
    let after = nl.len();
    assert_eq!(after + 2, before);
    let gates: Vec<_> = nl
        .objects()
        .filter(|n| !n.is_an_input())
        .filter(|n| n.get_instance_type().is_some())
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(
        gates[0].get_instance_type().unwrap().get_type(),
        CellType::AND4
    );
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
    let after = nl.len();
    assert_eq!(after + 1, before);
    let gates: Vec<_> = nl
        .objects()
        .filter(|n| !n.is_an_input())
        .filter(|n| n.get_instance_type().is_some())
        .collect();
    assert_eq!(gates.len(), 1);
    assert_eq!(
        gates[0].get_instance_type().unwrap().get_type(),
        CellType::OR3
    );
}

#[test]
fn test_monotone_fold_idempotent_after() {
    let nl = monotone_and_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let res1 = folder.run(&nl);
    assert!(res1.is_ok());
    let after_first = nl.len();
    let res2 = folder.run(&nl);
    assert!(res2.is_ok());
    assert_eq!(nl.len(), after_first);
    assert_eq!(res2.unwrap(), "Folded 1 patterns over 0 iterations");
}

#[test]
fn test_monotone_no_fold_shared_child() {
    let nl = monotone_no_fold_netlist();
    let mut folder = Folder::<Cell>::new(101);
    folder.insert(MonotoneFold);
    let before = nl.len();
    let res = folder.run(&nl);
    assert!(res.is_ok());
    assert_eq!(nl.len(), before - 1);
    assert_eq!(res.unwrap(), "Folded 1 patterns over 2 iterations");
}