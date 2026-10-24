//! Modo línea de comandos (útil por SSH o para automatizar).

use crate::catalog::{self, Format, Variant, APPS};
use crate::state::State;
use crate::system::SystemInfo;
use crate::{connect, github, installer, plan};

const HELP: &str = "iureapps — lanzador de las apps de Iurefficient

Uso:
  iureapps                         abre la ventana
  iureapps equipo                  muestra lo detectado del equipo
  iureapps lista                   apps, versión instalada y última
  iureapps instalar <app> [--variante cpu|cuda|vulkan|metal] [--formato appimage|deb|rpm]
  iureapps actualizar [<app>]      actualiza una app o todas las instaladas
  iureapps abrir <app>
  iureapps desinstalar <app>
  iureapps cuenta                  cuenta de Iurefficient compartida por las apps
  iureapps conectar <instancia> <correo>   inicia sesión (pide la contraseña)
  iureapps desconectar             cierra la sesión en este equipo
  iureapps iurefficient            abre la instancia en el navegador

Token para repos privados: variable IUREAPPS_GITHUB_TOKEN (o GITHUB_TOKEN).";

pub fn run(args: &[String]) -> i32 {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let result = match cmd {
        "-h" | "--help" | "ayuda" | "help" => {
            println!("{HELP}");
            Ok(())
        }
        "-V" | "--version" => {
            println!("iureapps {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "equipo" | "system" => {
            print_system(&SystemInfo::detect());
            Ok(())
        }
        "lista" | "list" => list(),
        "instalar" | "install" => install(&args[1..], false),
        "actualizar" | "update" => install(&args[1..], true),
        "abrir" | "run" | "open" => open(&args[1..]),
        "desinstalar" | "uninstall" => uninstall(&args[1..]),
        "cuenta" | "account" => account(),
        "conectar" | "login" => login(&args[1..]),
        "desconectar" | "logout" => logout(),
        "iurefficient" | "web" => open_web(),
        other => Err(format!("orden desconocida: {other}\n\n{HELP}")),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn print_system(sys: &SystemInfo) {
    println!("Sistema:  {}", sys.platform_label());
    println!(
        "CPU:      {} ({} hilos{})",
        sys.cpu,
        sys.cores,
        match sys.avx2 {
            Some(true) => ", AVX2",
            Some(false) => ", SIN AVX2",
            None => "",
        }
    );
    if let Some(r) = sys.ram_gb {
        println!("Memoria:  {r:.1} GB");
    }
    for g in &sys.gpus {
        println!("GPU:      {}", g.name);
    }
    if let Some(n) = &sys.nvidia {
        println!(
            "NVIDIA:   driver {} ({} MB)",
            n.driver,
            n.vram_mb.unwrap_or(0)
        );
    }
    println!("Vulkan:   {}", if sys.vulkan { "sí" } else { "no" });
    if sys.os == crate::system::Os::Linux {
        println!(
            "Paquetes: {:?}  FUSE: {}  Escritorio: {} {}",
            sys.pkg,
            if sys.fuse { "sí" } else { "no" },
            sys.desktop,
            sys.session
        );
    }
    if let Some(app) = catalog::find("iuretranscribe") {
        let (v, why) = app.recommend_variant(sys);
        println!("IureTranscribe recomendado: {} — {why}", v.label());
    }
}

fn app_arg(args: &[String]) -> Result<&'static catalog::AppDef, String> {
    let id = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or("falta el nombre de la app")?;
    catalog::find(id).ok_or_else(|| {
        let ids: Vec<_> = APPS.iter().map(|a| a.id).collect();
        format!("app desconocida: {id} (opciones: {})", ids.join(", "))
    })
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn list() -> Result<(), String> {
    let sys = SystemInfo::detect();
    let state = State::load();
    let token = github::token(state.github_token.as_deref());
    println!("{:<16} {:<12} {:<12} ESTADO", "APP", "INSTALADA", "ÚLTIMA");
    for app in APPS {
        let installed = installer::detect(app, &sys, state.installs.get(app.id));
        let release = github::latest_release(app.repo, token.as_deref());
        let st = plan::status(app, &sys, Some(&release), installed.as_ref());
        let inst = installed
            .as_ref()
            .map(|i| i.version.clone().unwrap_or_else(|| "?".into()))
            .unwrap_or_else(|| "—".into());
        let latest = match &release {
            Ok(r) => r.version().to_string(),
            Err(_) => "—".into(),
        };
        println!(
            "{:<16} {:<12} {:<12} {}",
            app.name,
            inst,
            latest,
            st.label()
        );
    }
    Ok(())
}

fn install(args: &[String], update: bool) -> Result<(), String> {
    let sys = SystemInfo::detect();
    let mut state = State::load();
    let token = github::token(state.github_token.as_deref());
    let targets: Vec<&catalog::AppDef> = if update && args.iter().all(|a| a.starts_with("--")) {
        APPS.iter()
            .filter(|a| installer::detect(a, &sys, state.installs.get(a.id)).is_some())
            .collect()
    } else {
        vec![app_arg(args)?]
    };
    for app in targets {
        let release = github::latest_release(app.repo, token.as_deref())
            .map_err(|e| format!("{}: {e}", app.name))?;
        let installed = installer::detect(app, &sys, state.installs.get(app.id));
        if update {
            let st = plan::status(app, &sys, Some(&Ok(release.clone())), installed.as_ref());
            if st != plan::Status::UpdateAvailable && st != plan::Status::NotInstalled {
                println!("{}: al día ({})", app.name, release.version());
                continue;
            }
        }
        let mut pref = state.prefs.get(app.id).cloned().unwrap_or_default();
        if let Some(v) = flag(args, "--variante").or(flag(args, "--variant")) {
            pref.variant =
                Some(Variant::parse(v).ok_or_else(|| format!("variante desconocida: {v}"))?);
        }
        if let Some(f) = flag(args, "--formato").or(flag(args, "--format")) {
            pref.format =
                Some(Format::parse(f).ok_or_else(|| format!("formato desconocido: {f}"))?);
        }
        let variant = plan::variant(app, &sys, installed.as_ref(), Some(&pref));
        let format = plan::format(
            app,
            &sys,
            &release,
            variant,
            installed.as_ref(),
            Some(&pref),
        )
        .ok_or_else(|| {
            format!(
                "{}: no hay instalador {} para este equipo",
                app.name,
                variant.label()
            )
        })?;
        println!(
            "{} {} — {} {}",
            app.name,
            release.version(),
            variant.label(),
            format.label()
        );
        let last = std::cell::Cell::new(-1i32);
        let managed = installer::install(
            app,
            &release,
            &sys,
            variant,
            format,
            token.as_deref(),
            state.installs.get(app.id),
            &|m| println!("  {m}"),
            &|p| {
                let pct = (p * 100.0) as i32;
                if pct / 10 != last.get() / 10 {
                    last.set(pct);
                    println!("  {pct}%");
                }
            },
        )?;
        state.installs.insert(app.id.to_string(), managed);
        state.save();
        println!("  listo");
    }
    Ok(())
}

fn open(args: &[String]) -> Result<(), String> {
    let app = app_arg(args)?;
    let sys = SystemInfo::detect();
    let state = State::load();
    let installed =
        installer::detect(app, &sys, state.installs.get(app.id)).ok_or("no está instalada")?;
    installer::launch(&installed, &sys)
}

fn uninstall(args: &[String]) -> Result<(), String> {
    let app = app_arg(args)?;
    let sys = SystemInfo::detect();
    let mut state = State::load();
    let installed =
        installer::detect(app, &sys, state.installs.get(app.id)).ok_or("no está instalada")?;
    installer::uninstall(app, &installed)?;
    state.installs.remove(app.id);
    state.save();
    println!("{} desinstalada", app.name);
    Ok(())
}

fn account() -> Result<(), String> {
    let Some(s) = connect::saved() else {
        println!("Sin cuenta de Iurefficient en este equipo (usa `iureapps conectar`).");
        return Ok(());
    };
    println!("Instancia: {}", s.web_url);
    println!("Correo:    {}", s.email);
    if !s.app.is_empty() {
        println!("La dejó:   {}", s.app);
    }
    println!(
        "WebDAV:    {}",
        if s.has_webdav {
            "contraseña guardada"
        } else {
            "sin contraseña"
        }
    );
    if !s.has_session {
        println!("Sesión:    no hay (vuelve a conectar)");
        return Ok(());
    }
    match connect::verify(&s) {
        Ok(name) => println!("Sesión:    válida, {name}"),
        Err(e) => println!("Sesión:    caducada ({e})"),
    }
    Ok(())
}

fn read_line(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

fn login(args: &[String]) -> Result<(), String> {
    let (Some(domain), Some(email)) = (args.first(), args.get(1)) else {
        return Err("uso: iureapps conectar <instancia> <correo>".into());
    };
    let password = match std::env::var("IUREAPPS_PASSWORD") {
        Ok(p) => p,
        Err(_) => read_line("Contraseña (se verá al escribir): ")?,
    };
    let outcome = match connect::login(domain, email, &password, None)? {
        connect::LoginOutcome::TotpRequired { totp_token } => {
            let code = read_line("Código de verificación: ")?;
            connect::login(domain, email, &password, Some((&totp_token, &code)))?
        }
        ok => ok,
    };
    if let connect::LoginOutcome::Ok {
        name,
        created_webdav,
    } = outcome
    {
        println!("Conectada como {name}.");
        if created_webdav {
            println!("Se creó la contraseña de aplicación WebDAV de este equipo.");
        }
        println!("Las apps sin cuenta configurada la usarán al abrirse.");
    }
    Ok(())
}

fn logout() -> Result<(), String> {
    let s = connect::saved().ok_or("no hay cuenta conectada")?;
    connect::logout(&s)?;
    println!("Sesión cerrada en este equipo.");
    Ok(())
}

fn open_web() -> Result<(), String> {
    let s = connect::saved().ok_or("no hay cuenta conectada (usa `iureapps conectar`)")?;
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    crate::util::command(opener)
        .arg(&s.web_url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("no se pudo abrir el navegador: {e}"))
}
