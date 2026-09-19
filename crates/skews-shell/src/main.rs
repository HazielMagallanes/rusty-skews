//! `rusty-skews`: a skewed, Rust-native Wayland desktop shell.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use skews_app::{ModuleOutput, ModuleSlot, Msg, Shell};
use skews_config::{Config, default_config_path};
use skews_core::{Effect, Effects, LogLevel};
use skews_layout::{BarLayoutOptions, TextMetrics};
use skews_render::{GpuSurface, Renderer};
use skews_text::TextEngine;
use skews_theme::{Tokens, default_palette_path};
use skews_wayland::{
    BarEvent, BarOptions, BarShell, Connection, ObjectId, display_handle, registry_queue_init,
    window_handle,
};
use tracing::{debug, error, info, warn};

/// Command line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rusty-skews",
    version,
    about = "A skewed, Rust-native Wayland shell"
)]
struct Args {
    /// Path to the configuration file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Increase log verbosity (`-v`, `-vv`).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Development helper: exit successfully after N seconds.
    #[arg(long, value_name = "SECONDS", hide = true)]
    exit_after: Option<u64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose);
    run(&args)
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let default_filter =
        format!("rusty_skews={level},skews_wayland={level},skews_render={level},warn");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();
}

fn run(args: &Args) -> Result<()> {
    let (config, config_path) = load_config(args.config.clone())?;
    let theme_path = default_palette_path();
    let theme = load_theme(&theme_path);
    let mut text_engine = TextEngine::new(theme.font_family.clone());

    let mut registry = skews_app::Registry::new();
    skews_modules::register_all(&mut registry);
    let mut shell = Shell::new(config, registry).context("invalid configuration")?;

    // Seed modules with their first value so the bar plan is complete before
    // the event loop starts. Live ticking arrives with the calloop runtime.
    apply_effects(shell.update(Msg::Tick { unix_ms: unix_ms() }));

    info!(
        config = %config_path.display(),
        height = shell.state().bar.height,
        monitor = %shell.state().bar.monitor,
        revision = shell.state().config_revision,
        "configuration loaded"
    );

    if let Some(seconds) = args.exit_after {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            std::process::exit(0);
        });
    }

    let conn = Connection::connect_to_env()
        .map_err(|error| anyhow::anyhow!("failed to connect to the Wayland compositor: {error}"))?;
    let (globals, mut queue) = registry_queue_init::<BarShell>(&conn)
        .context("failed to initialize the Wayland registry")?;
    let qh = queue.handle();

    let mut bar = BarShell::bind(
        &globals,
        &qh,
        BarOptions {
            height: shell.state().bar.height,
            monitor: shell.state().bar.monitor.clone(),
            namespace: String::from("rusty-skews-bar"),
        },
    )
    .context("failed to bind bar surfaces")?;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

    let mut renderer: Option<Renderer> = None;
    // ObjectId is the documented stable identity for Wayland objects; the
    // interior mutability clippy sees is a liveness flag, not part of the hash.
    #[allow(
        clippy::mutable_key_type,
        reason = "ObjectId hashes its stable protocol identity (wayland-client docs endorse map keys)"
    )]
    let mut surfaces: HashMap<ObjectId, GpuSurface> = HashMap::new();

    info!("entering Wayland event loop");

    loop {
        queue
            .blocking_dispatch(&mut bar)
            .map_err(|error| anyhow::anyhow!("Wayland dispatch failed: {error}"))?;

        for event in bar.drain_events() {
            match event {
                BarEvent::Created {
                    id,
                    output,
                    surface: _,
                    scale,
                } => {
                    info!(surface = ?id, ?output, scale, "bar surface created");
                }
                BarEvent::Configured { id, width, height } => {
                    if !surfaces.contains_key(&id) {
                        let wl_surface = bar
                            .wl_surface(&id)
                            .context("configured surface disappeared before first frame")?;
                        let created = GpuSurface::create(
                            &instance,
                            display_handle(&conn),
                            window_handle(wl_surface),
                        )
                        .context("failed to create the GPU surface")?;
                        surfaces.insert(id.clone(), created);
                    }

                    let gpu = surfaces.get_mut(&id).expect("surface was just created");

                    if renderer.is_none() {
                        let created = Renderer::new(&instance, Some(gpu.wgpu_surface()))
                            .context("no usable GPU adapter found")?;
                        renderer = Some(created);
                    }
                    let renderer = renderer.as_mut().expect("renderer was just created");

                    renderer
                        .configure_surface(gpu, width, height)
                        .context("failed to configure the GPU surface")?;

                    let scene = build_scene(&shell, &mut text_engine, &theme, width, height)
                        .context("failed to build the bar scene")?;
                    renderer
                        .render_scene(gpu, &scene, &mut text_engine)
                        .context("failed to render the bar")?;

                    info!(surface = ?id, width, height, "bar configured");

                    let plan = shell.bar_plan();
                    info!(
                        left = %describe(&plan.left),
                        center = %describe(&plan.center),
                        right = %describe(&plan.right),
                        "bar plan"
                    );
                }
                BarEvent::Closed { id } => {
                    surfaces.remove(&id);
                    info!(surface = ?id, "bar surface closed");
                    if surfaces.is_empty() && renderer.is_some() {
                        info!("all bar surfaces closed; exiting");
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn load_config(explicit: Option<PathBuf>) -> Result<(Config, PathBuf)> {
    let path = explicit.unwrap_or_else(default_config_path);

    match Config::load(&path) {
        Ok(config) => Ok((config, path)),
        Err(skews_config::ConfigError::Io { .. }) => {
            warn!(config = %path.display(), "configuration file not found; using defaults");
            Ok((Config::default(), path))
        }
        Err(error) => Err(error).context("invalid configuration"),
    }
}

fn load_theme(path: &std::path::Path) -> Tokens {
    match Tokens::load(path) {
        Ok(tokens) => tokens,
        Err(error) => {
            warn!(theme = %path.display(), %error, "palette not loaded; using defaults");
            Tokens::default()
        }
    }
}

/// Executes kernel effects (the runtime half of effects-as-data).
fn apply_effects(effects: Effects) {
    for effect in effects {
        match effect {
            Effect::Log { level, message } => match level {
                LogLevel::Debug => debug!(target: "rusty_skews::kernel", "{message}"),
                LogLevel::Info => info!(target: "rusty_skews::kernel", "{message}"),
                LogLevel::Warn => warn!(target: "rusty_skews::kernel", "{message}"),
                LogLevel::Error => error!(target: "rusty_skews::kernel", "{message}"),
            },
            Effect::Redraw => debug!(target: "rusty_skews::kernel", "redraw requested"),
        }
    }
}

/// Builds the bar scene from the kernel plan, the layout pass and the theme.
fn build_scene(
    shell: &Shell,
    text_engine: &mut TextEngine,
    theme: &Tokens,
    width: u32,
    height: u32,
) -> Result<skews_ui::Scene> {
    const TEXT_SIZE: f32 = 12.0;

    let plan = shell.bar_plan();
    let mut measure = |slots: &[ModuleSlot]| -> Vec<(String, TextMetrics)> {
        slots
            .iter()
            .filter_map(|slot| match &slot.output {
                ModuleOutput::Text(text) => {
                    let shaped = text_engine.measure(text, TEXT_SIZE);
                    Some((
                        text.clone(),
                        TextMetrics {
                            width: shaped.width,
                            height: shaped.height,
                        },
                    ))
                }
                ModuleOutput::Empty => None,
            })
            .collect()
    };

    let left = measure(&plan.left);
    let center = measure(&plan.center);
    let right = measure(&plan.right);

    let metrics = |items: &[(String, TextMetrics)]| {
        items
            .iter()
            .map(|(_, metrics)| *metrics)
            .collect::<Vec<_>>()
    };
    let layout = skews_layout::layout_bar(
        BarLayoutOptions::new(width as f32, height as f32),
        &metrics(&left),
        &metrics(&center),
        &metrics(&right),
    )
    .context("layout pass failed")?;

    let mut texts = Vec::new();
    for (positions, items) in [
        (&layout.left, &left),
        (&layout.center, &center),
        (&layout.right, &right),
    ] {
        for (position, (text, _)) in positions.iter().zip(items.iter()) {
            texts.push(skews_ui::BarText {
                text: text.clone(),
                x: position.x,
                y: position.y,
                size: TEXT_SIZE,
                color: theme.on_surface,
            });
        }
    }

    Ok(skews_ui::build_bar_scene(
        theme.surface_container.with_alpha(0.9),
        &texts,
    ))
}

/// Human-readable description of a bar region's module slots.
fn describe(slots: &[ModuleSlot]) -> String {
    if slots.is_empty() {
        return String::from("-");
    }

    slots
        .iter()
        .map(|slot| match &slot.output {
            ModuleOutput::Empty => format!("{}:empty", slot.id),
            ModuleOutput::Text(text) => format!("{}={text}", slot.id),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Milliseconds since the Unix epoch.
fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}
