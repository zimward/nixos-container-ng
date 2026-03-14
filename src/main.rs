use std::{
    fs::ReadDir,
    os::fd::{AsRawFd, FromRawFd},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use clap::{Args, Parser, Subcommand};
use zbus::{blocking::Connection, zvariant::OwnedObjectPath};

use crate::{systemd_manager::ManagerProxyBlocking, unit::UnitProxyBlocking};

mod systemd_manager {
    include!(concat!(env!("OUT_DIR"), "/systemd_manager.rs"));
}
mod unit {
    include!(concat!(env!("OUT_DIR"), "/unit.rs"));
}

mod machine_manager {
    include!(concat!(env!("OUT_DIR"), "/machine_manager.rs"));
}

mod job {
    include!(concat!(env!("OUT_DIR"), "/job.rs"));
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

fn main() -> zbus::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List => {
            list();
            Ok(())
        }
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
        Commands::Login { container_name } => login(&container_name),
        Commands::RootLogin { container_name } => root_login(&container_name),
        Commands::Run {
            container_name,
            args,
        } => todo!(),
        Commands::ShowIp { container_name } => todo!(),
        Commands::ShowHostKey { container_name } => todo!(),
    }
}

fn list() {
    let config_dir = Path::new(CONFIG_DIR);

    let entries: ReadDir = match config_dir.read_dir() {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!(
                "Failed to read config directory {}: {e}",
                config_dir.display()
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension() != Some("conf".as_ref()) {
            continue;
        }

        let name = match path.file_stem() {
            Some(name) => name.to_string_lossy(),
            None => continue,
        };
        // since we already ensured its a .conf file no need to check full name
        if matches!(name.as_ref(), "libpod" | "containers" | "registries") {
            continue;
        }

        println!("{name}");
    }
}

fn get_container_path(container_name: &str, conn: &Connection) -> zbus::Result<OwnedObjectPath> {
    let systemd = ManagerProxyBlocking::new(conn)?;
    systemd.get_unit(&format!("container@{container_name}.service"))
}

fn status(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");
    let unit = UnitProxyBlocking::builder(&conn)
        .path(get_container_path(container_name, &conn)?)
        .unwrap()
        .build()
        .unwrap();
    if unit.active_state().expect("Failed to query to unit") == "active" {
        println!("up");
    } else {
        println!("down");
    }
    Ok(())
}

// TODO: show "no such container" message if the unit isn't known

fn start(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");

    let systemd = ManagerProxyBlocking::new(&conn)?;
    let jobrm = systemd.receive_job_removed()?;
    // lets just assume the unit has started correctly if the job doesn't error
    match systemd.start_unit(&format!("container@{container_name}.service"), "fail") {
        Ok(res) => {
            for job in jobrm {
                if job.args()?.job == *res {
                    break;
                }
            }
            //maybe check status of unit here and return non-zero exit code
        }
        Err(e) => {
            dbg!(e);
            eprintln!("Failed to start container unit {container_name}.");
        }
    }
    Ok(())
}

fn stop(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");

    let systemd = ManagerProxyBlocking::new(&conn)?;
    let jobrm = systemd.receive_job_removed()?;

    match systemd.stop_unit(&format!("container@{container_name}.service"), "fail") {
        Ok(res) => {
            for job in jobrm {
                if job.args()?.job == *res {
                    break;
                }
            }
        }
        Err(e) => {
            dbg!(e);
            eprintln!("Failed to stop container unit {container_name}.");
        }
    }
    Ok(())
}

fn restart(container_name: &str) -> zbus::Result<()> {
    stop(container_name)?;
    start(container_name)
}

// does an unclean immediate shutdown instead of a stop
fn terminate(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");
    let machined = machine_manager::ManagerProxyBlocking::new(&conn)
        .expect("Failed to connect to machine manager");

    machined.terminate_machine(container_name)
}

fn login(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");
    let machined = machine_manager::ManagerProxyBlocking::new(&conn)
        .expect("Failed to connect to machine manager");

    let pty = machined.open_machine_login(container_name)?;
    Ok(())
}

fn root_login(container_name: &str) -> zbus::Result<()> {
    let conn = Connection::system().expect("Failed to connect to dbus");
    let machined = machine_manager::ManagerProxyBlocking::new(&conn)
        .expect("Failed to connect to machine manager");
    let pty = {
        //make sure the object is not accessible once the rawfd has been extracted
        let pty =
            machined.open_machine_shell(container_name, "root", "/usr/bin/env", &["bash"], &[])?;
        unsafe { Stdio::from_raw_fd(pty.0.as_raw_fd()) }
    };
    //connect the pty to the terminal and exit?
    Ok(())
}
