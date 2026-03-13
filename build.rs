use std::{ffi::OsString, io::Write, process::Command};

fn code_for_dbus_xml(xml: impl Into<OsString>, service: &str, default_path: &str) -> String {
    let output = Command::new("zbus-xmlgen")
        .arg("file")
        .arg(xml.into())
        .arg("-o")
        .arg("-")
        .output()
        .expect("Failed generating code from xml");
    let source =
        String::from_utf8(output.stdout).expect("generated XML contained illegal codepoints");
    //remove doc comments
    let source = source.replace("//!", "///");
    //patch proxy macro
    source.replace(
        "assume_defaults = true",
        format!("assume_defaults =true,default_service = \"{service}\", default_path = \"{default_path}\"")
            .as_str(),
    )
}

fn add_interactive_auth(src: String, methods: &[&str]) -> String {
    let mut out = src;
    for m in methods {
        out.insert_str(
            out.find(&format!("fn {m}")).expect("method not found"),
            "\n#[zbus(allow_interactive_auth)]\n",
        );
    }
    out
}

fn main() {
    let systemd_dbus_interface_dir = std::env::var("SYSTEMD_DBUS_INTERFACE_DIR").unwrap();
    let systemd_dbus_interface_dir = std::path::Path::new(systemd_dbus_interface_dir.as_str());

    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let systemd_service = "org.freedesktop.systemd1";
    let systemd_path = "/org/freedesktop/systemd1";

    let systemd_manager_code = code_for_dbus_xml(
        systemd_dbus_interface_dir.join("org.freedesktop.systemd1.Manager.xml"),
        systemd_service,
        systemd_path,
    );
    let systemd_manager_code =
        add_interactive_auth(systemd_manager_code, &["start_unit", "stop_unit"]);
    let mut file = std::fs::File::create(out_path.join("systemd_manager.rs")).unwrap();
    file.write_all(systemd_manager_code.as_bytes()).unwrap();

    let unit_code = code_for_dbus_xml(
        systemd_dbus_interface_dir.join("org.freedesktop.systemd1.Unit.xml"),
        systemd_service,
        systemd_path,
    );
    let mut file = std::fs::File::create(out_path.join("unit.rs")).unwrap();
    file.write_all(unit_code.as_bytes()).unwrap();

    let machine_manager_code = code_for_dbus_xml(
        systemd_dbus_interface_dir.join("org.freedesktop.machine1.Manager.xml"),
        systemd_service,
        systemd_path,
    );
    let mut file = std::fs::File::create(out_path.join("machine_manager.rs")).unwrap();
    file.write_all(machine_manager_code.as_bytes()).unwrap();
}
