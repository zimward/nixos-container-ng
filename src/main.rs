use std::{ops::IndexMut, path::PathBuf, time::Duration};

use clap::{command, Arg, Args, Parser, Subcommand};
use dbus::{
    blocking::{stdintf::org_freedesktop_dbus::ObjectManager, LocalConnection, Proxy},
    Message, Path,
};
use glob::glob;
use nix::libc::ELIBEXEC;

use crate::{
    machine_manager::OrgFreedesktopMachine1Manager, systemd_manager::OrgFreedesktopSystemd1Manager,
    unit::OrgFreedesktopSystemd1Unit,
};

mod systemd_manager {
    include!(concat!(env!("OUT_DIR"), "/systemd_manager.rs"));
}
mod unit {
    include!(concat!(env!("OUT_DIR"), "/unit.rs"));
}

mod machine_manager {
    include!(concat!(env!("OUT_DIR"), "/machine_manager.rs"));
}

static BUS_TIMEOUT: Duration = Duration::from_secs(25);

#[cfg(not(debug_assertions))]
static CONFIG_DIR: &str = env!("CONFIGURATION_DIR");
#[cfg(debug_assertions)]
static CONFIG_DIR: &str = "/etc/nixos-containers";

#[cfg(not(debug_assertions))]
static STATE_DIR: &str = env!("STATE_DIR");
#[cfg(debug_assertions)]
static STATE_DIR: &str = "/var/lib/nixos-containers";

#[derive(Args, Clone)]
#[group(requires_all = ["host_address","local_address"])]
struct AddrArgs {
    #[arg(long)]
    host_address: Option<String>,
    #[arg(long)]
    local_address: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Create {
        container_name: String,
        #[arg(short, long)]
        nixos_path: Option<PathBuf>,
        #[arg(short, long)]
        system_path: Option<PathBuf>,
        #[arg(short, long, conflicts_with = "config_file")]
        config: Option<PathBuf>,
        #[arg(long, conflicts_with = "config")]
        config_file: Option<PathBuf>,
        #[arg(short, long)]
        flake: Option<String>,
        #[arg(short, long)]
        ensure_unique_name: bool,
        #[arg(short, long)]
        auto_start: bool,
        #[arg(long)]
        bridge: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[command(flatten)]
        addresses: AddrArgs,
        #[arg(long, conflicts_with_all = ["host_address","local_address"])]
        use_host_network: bool,
    },
    Destroy {
        container_name: String,
    },
    Restart {
        container_name: String,
    },
    Start {
        container_name: String,
    },
    Stop {
        container_name: String,
    },
    Terminate {
        container_name: String,
    },
    Status {
        container_name: String,
    },
    Update {
        container_name: String,
        #[arg(short, long, conflicts_with = "config_file")]
        config: Option<PathBuf>,
        #[arg(long, conflicts_with = "config")]
        config_file: Option<PathBuf>,
        #[arg(short, long)]
        flake: Option<String>,
        #[arg(short, long)]
        nixos_path: Option<PathBuf>,
        #[arg(short, long)]
        refresh: bool,
    },
    Login {
        container_name: String,
    },
    RootLogin {
        container_name: String,
    },
    Run {
        container_name: String,
        args: Vec<String>,
    },
    ShowIp {
        container_name: String,
    },
    ShowHostKey {
        container_name: String,
    },
}

#[derive(Parser)]
#[command(version,about,long_about=None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::List => list(),
        Commands::Create {
            container_name,
            nixos_path,
            system_path,
            config,
            config_file,
            flake,
            ensure_unique_name,
            auto_start,
            bridge,
            port,
            addresses,
            use_host_network,
        } => todo!(),
        Commands::Destroy { container_name } => todo!(),
        Commands::Restart { container_name } => restart(&container_name),
        Commands::Start { container_name } => start(&container_name),
        Commands::Stop { container_name } => stop(&container_name),
        Commands::Terminate { container_name } => terminate(&container_name),
        Commands::Status { container_name } => status(&container_name),
        Commands::Update {
            container_name,
            config,
            config_file,
            flake,
            nixos_path,
            refresh,
        } => todo!(),
        Commands::Login { container_name } => todo!(),
        Commands::RootLogin { container_name } => todo!(),
        Commands::Run {
            container_name,
            args,
        } => todo!(),
        Commands::ShowIp { container_name } => todo!(),
        Commands::ShowHostKey { container_name } => todo!(),
    }
}

fn list() {
    let config_files = glob(&format!("{CONFIG_DIR}/*.conf")).expect("Faild to read config dir");
    let config_files = config_files.filter_map(Result::ok).filter(|it| {
        it == "/etc/containers/libpod.conf"
            || it == "/etc/containers/containers.conf"
            || it == "/etc/containers/registries.conf"
    });
    for f in config_files {
        if let Some(f) = f.file_prefix() {
            println!("{}", f.to_string_lossy());
        }
    }
}

fn systemd_connect(conn: &LocalConnection) -> Proxy<'static, &LocalConnection> {
    let systemd = Proxy::new(
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        BUS_TIMEOUT,
        conn,
    );
    systemd
        .subscribe()
        .expect("Failed to subscribe to systemd dbus messages");
    systemd
}

fn get_container_path(container_name: &str, conn: &LocalConnection) -> Path<'static> {
    let systemd = systemd_connect(&conn);
    systemd
        .get_unit(&format!("container@{container_name}.service"))
        .expect("failed to get unit")
}

fn status(container_name: &str) {
    let conn = LocalConnection::new_system().expect("Failed to connect to dbus");
    let unit = conn.with_proxy(
        "org.freedesktop.systemd1",
        get_container_path(container_name, &conn),
        BUS_TIMEOUT,
    );
    if unit.active_state().expect("Failed to query to unit") == "active" {
        println!("up");
    } else {
        println!("down");
    }
}

// TODO: show "no such container" message if the unit isn't known

fn start(container_name: &str) {
    let conn = LocalConnection::new_system().expect("Failed to connect to dbus");

    let systemd = systemd_connect(&conn);

    match systemd.start_unit(&format!("container@{container_name}.service"), "fail") {
        Ok(res) => {
            dbg!(&res);
            let unit = conn.with_proxy("org.freedesktop.systemd1", res, BUS_TIMEOUT);
            // TODO: check job state
            // dbg!(unit.active_state().unwrap());
        }
        Err(e) => {
            eprintln!("Failed to start container unit {container_name}. Try running as root as interactive auth isn't implemented yet");
        }
    }
}

fn stop(container_name: &str) {
    let conn = LocalConnection::new_system().expect("Failed to connect to dbus");

    let systemd = systemd_connect(&conn);

    match systemd.stop_unit(&format!("container@{container_name}.service"), "fail") {
        Ok(res) => {
            dbg!(&res);
            let unit = conn.with_proxy("org.freedesktop.systemd1", res, BUS_TIMEOUT);
            // dbg!(unit.active_state().unwrap());
        }
        Err(e) => {
            eprintln!("Failed to stop container unit {container_name}. Try running as root as interactive auth isn't implemented yet");
        }
    }
}

fn restart(container_name: &str) {
    stop(container_name);
    start(container_name);
}

// does an unclean immediate shutdown instead of a stop
fn terminate(container_name: &str) {
    let conn = LocalConnection::new_system().expect("Failed to connect to dbus");
    let machined = Proxy::new(
        "org.freedesktop.machine1",
        "/org/freedesktop/machine1",
        BUS_TIMEOUT,
        &conn,
    );

    machined
        .terminate_machine(container_name)
        .expect("Failed to terminate machine. Try running as root as interactive auth isn't implemented yet");
}

fn login(container_name: &str) {}
