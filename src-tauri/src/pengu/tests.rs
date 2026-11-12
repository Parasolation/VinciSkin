use super::*;

#[test]
fn the_debugger_entry_runs_the_dll_by_ordinal() {
    let value = debugger_value(Path::new(r"C:\data\runtime\pengu\core.dll"));

    assert_eq!(
        value.trim(),
        r#"rundll32 "C:\data\runtime\pengu\core.dll", #6000"#
    );
}

#[test]
fn an_entry_written_elsewhere_is_not_the_same_command() {
    let ours = debugger_value(Path::new(r"C:\data\runtime\pengu\core.dll"));

    assert!(!same_command(
        &ours,
        r#"rundll32 "C:\other\core.dll", #6000"#
    ));
}

#[test]
fn an_entry_written_with_other_spacing_and_case_is_recognised() {
    let ours = debugger_value(Path::new(r"C:\data\runtime\pengu\core.dll"));

    assert!(same_command(
        &ours,
        r#"  RUNDLL32 "c:\data\runtime\pengu\core.dll", #6000"#
    ));
}

#[test]
fn an_older_window_registration_names_this_runtime() {
    let dir = Path::new(r"C:\data\runtime\pengu");
    let older = r"C:\data\runtime\pengu\Pengu Loader.exe";

    assert!(path_contains(older, dir));
    assert!(!same_command(older, &debugger_value(&dir.join(CORE_DLL))));
}
