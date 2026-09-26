//! `rusty-skews`: a skewed, Rust-native Wayland desktop shell.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use calloop::EventLoop;
use calloop::channel::{Event as ChannelEvent, Sender, channel};
use calloop::timer::{TimeoutAction, Timer};
use calloop_wayland_source::WaylandSource;
use clap::Parser;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use skews_app::{ModuleOutput, ModuleSlot, Msg, Shell};
use skews_config::{Config, default_config_path};
use skews_core::{Effect, Effects, LogLevel};
use skews_ipc_hyprland::{HyprEvent, active_workspace, event_socket_path, spawn_event_listener};
use skews_layout::{BarLayoutOptions, TextMetrics};
use skews_render::{GpuSurface, Renderer};
use skews_text::TextEngine;
use skews_theme::{Tokens, default_palette_path};
use skews_wayland::{
    BarEvent, BarOptions, BarShell, Connection, EventQueue, ObjectId, display_handle,
    registry_queue_init, window_handle,
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

/// Tick cadence for clocks and sensors.
///
/// The clock only needs minute precision and the performance budget treats
/// sensor polling of a few seconds as fine; a slower tick directly reduces
/// redraws and idle CPU.
const TICK_INTERVAL: Duration = Duration::from_secs(2);

/// Per-surface GPU state.
struct SurfaceState {
    gpu: GpuSurface,
    width: u32,
    height: u32,
    /// Size the swapchain was last configured for.
    configured: Option<(u32, u32)>,
}

/// Runtime state shared by the calloop sources.
struct Runtime {
    shell: Shell,
    theme: Tokens,
    text_engine: TextEngine,
    instance: wgpu::Instance,
    renderer: Option<Renderer>,
    surfaces: HashMap<ObjectId, SurfaceState>,
}

impl Runtime {
    fn new(shell: Shell, theme: Tokens) -> Self {
        let text_engine = TextEngine::new(theme.font_family.clone());
        let instance = create_instance();

        Self {
            shell,
            theme,
            text_engine,
            instance,
            renderer: None,
            surfaces: HashMap::new(),
        }
    }

    /// Applies a kernel message, updates the bar if needed and renders.
    fn dispatch(&mut self, msg: Msg, bar: &mut BarShell) {
        let previous_height = self.shell.state().bar.height;
        let effects = self.shell.update(msg);
        let height = self.shell.state().bar.height;

        if height != previous_height {
            bar.set_bar_height(height);
            info!(height, "bar height changed; surfaces will reconfigure");
        }

        self.apply(effects);
    }

    fn apply(&mut self, effects: Effects) {
        let mut redraw = false;
        for effect in effects {
            match effect {
                Effect::Log { level, message } => log_effect(level, &message),
                Effect::Redraw => redraw = true,
            }
        }

        if redraw {
            self.render_all();
        }
    }

    fn handle_wayland(&mut self, conn: &Connection, bar: &mut BarShell) {
        for event in bar.drain_events() {
            match event {
                BarEvent::Created {
                    id, output, scale, ..
                } => {
                    info!(surface = ?id, ?output, scale, "bar surface created");
                }
                BarEvent::Configured { id, width, height } => {
                    if let Err(error) = self.on_configured(conn, bar, &id, width, height) {
                        error!(surface = ?id, %error, "failed to render a configured surface");
                    }
                }
                BarEvent::Closed { id } => {
                    self.surfaces.remove(&id);
                    info!(surface = ?id, "bar surface closed");
                }
            }
        }
    }

    fn on_configured(
        &mut self,
        conn: &Connection,
        bar: &mut BarShell,
        id: &ObjectId,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if !self.surfaces.contains_key(id) {
            let wl_surface = bar
                .wl_surface(id)
                .context("configured surface disappeared before the first frame")?;
            let gpu = GpuSurface::create(
                &self.instance,
                display_handle(conn),
                window_handle(wl_surface),
            )
            .context("failed to create the GPU surface")?;

            if self.renderer.is_none() {
                let renderer = Renderer::new(&self.instance, Some(gpu.wgpu_surface()))
                    .context("no usable GPU adapter found")?;
                self.renderer = Some(renderer);
            }

            self.surfaces.insert(
                id.clone(),
                SurfaceState {
                    gpu,
                    width,
                    height,
                    configured: None,
                },
            );
        }

        if let Some(state) = self.surfaces.get_mut(id) {
            state.width = width;
            state.height = height;
        }

        self.render_surface(id)
            .context("failed to render the bar")?;

        info!(surface = ?id, width, height, "bar configured");
        let plan = self.shell.bar_plan();
        info!(
            left = %describe(&plan.left),
            center = %describe(&plan.center),
            right = %describe(&plan.right),
            "bar plan"
        );

        Ok(())
    }

    fn render_all(&mut self) {
        let ids: Vec<ObjectId> = self.surfaces.keys().cloned().collect();
        for id in ids {
            if let Err(error) = self.render_surface(&id) {
                error!(surface = ?id, %error, "redraw failed");
            }
        }
    }

    fn render_surface(&mut self, id: &ObjectId) -> Result<()> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Ok(());
        };
        let Some(state) = self.surfaces.get_mut(id) else {
            return Ok(());
        };

        // Reconfiguring the swapchain is expensive; only do it when the size
        // actually changed (or on the first frame).
        let size = (state.width, state.height);
        if state.configured != Some(size) {
            renderer
                .configure_surface(&mut state.gpu, state.width, state.height)
                .context("failed to configure the GPU surface")?;
            state.configured = Some(size);
        }

        let scene = build_scene(
            &self.shell,
            &mut self.text_engine,
            &self.theme,
            state.width,
            state.height,
        )?;
        renderer
            .render_scene(&mut state.gpu, &scene, &mut self.text_engine)
            .context("failed to render the scene")?;

        Ok(())
    }
}

fn run(args: &Args) -> Result<()> {
    let (config, config_path) = load_config(args.config.clone())?;
    let theme = load_theme(&default_palette_path());

    let mut registry = skews_app::Registry::new();
    skews_modules::register_all(&mut registry);
    let shell = Shell::new(config.clone(), registry).context("invalid configuration")?;

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
    let (globals, queue) = registry_queue_init::<BarShell>(&conn)
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

    let mut event_loop: EventLoop<BarShell> =
        EventLoop::try_new().context("failed to create the event loop")?;
    let handle = event_loop.handle();
    let runtime = Rc::new(RefCell::new(Runtime::new(shell, theme)));

    // Seed modules with their first values.
    runtime
        .borrow_mut()
        .dispatch(Msg::Tick { unix_ms: unix_ms() }, &mut bar);

    // Wayland events: dispatch to the bar shell, then react to its events.
    {
        let runtime = Rc::clone(&runtime);
        let conn = conn.clone();
        handle
            .insert_source(
                WaylandSource::new(conn.clone(), queue),
                move |(), queue: &mut EventQueue<BarShell>, bar: &mut BarShell| {
                    let dispatched = queue.dispatch_pending(bar);
                    runtime.borrow_mut().handle_wayland(&conn, bar);
                    dispatched
                },
            )
            .context("failed to register the Wayland event source")?;
    }

    // Kernel messages: config reloads and compositor events.
    let (msg_tx, msg_channel) = channel::<Msg>();
    {
        let runtime = Rc::clone(&runtime);
        handle
            .insert_source(msg_channel, move |event, (), bar: &mut BarShell| {
                if let ChannelEvent::Msg(msg) = event {
                    runtime.borrow_mut().dispatch(msg, bar);
                }
            })
            .map_err(|error| {
                anyhow::anyhow!("failed to register the message channel: {error:?}")
            })?;
    }

    // Tick timer: clock, sensors.
    {
        let runtime = Rc::clone(&runtime);
        let timer = Timer::from_duration(TICK_INTERVAL);
        handle
            .insert_source(timer, move |_instant, (), bar: &mut BarShell| {
                runtime
                    .borrow_mut()
                    .dispatch(Msg::Tick { unix_ms: unix_ms() }, bar);
                TimeoutAction::ToDuration(TICK_INTERVAL)
            })
            .map_err(|error| anyhow::anyhow!("failed to register the tick timer: {error:?}"))?;
    }

    // Configuration hot reload.
    let _config_watcher = setup_config_watcher(&config_path, &config, msg_tx.clone())?;

    // Compositor integration.
    setup_hyprland(&runtime, &mut bar, &msg_tx);

    info!("entering Wayland event loop");
    event_loop
        .run(None, &mut bar, |_| {})
        .context("the event loop failed")?;

    Ok(())
}

/// Watches the configuration file and forwards validated reloads as messages.
fn setup_config_watcher(
    config_path: &Path,
    current: &Config,
    sender: Sender<Msg>,
) -> Result<Option<RecommendedWatcher>> {
    let watch_target = if config_path.exists() {
        config_path.to_path_buf()
    } else if let Some(parent) = config_path.parent().filter(|parent| parent.exists()) {
        parent.to_path_buf()
    } else {
        warn!(config = %config_path.display(), "configuration directory missing; hot reload disabled");
        return Ok(None);
    };

    let path = config_path.to_path_buf();
    let mut last_config: Option<Config> = Some(current.clone());
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        if !event.paths.is_empty() && !event.paths.iter().any(|candidate| candidate == &path) {
            return;
        }

        match Config::load(&path) {
            Ok(config) => {
                // Editors write files in several steps; skip identical reloads.
                if last_config.as_ref() == Some(&config) {
                    return;
                }
                last_config = Some(config.clone());
                let _ = sender.send(Msg::ConfigLoaded(Box::new(config)));
            }
            Err(error) => {
                warn!(config = %path.display(), %error, "configuration reload rejected");
            }
        }
    })
    .context("failed to create the configuration watcher")?;

    watcher
        .watch(&watch_target, RecursiveMode::NonRecursive)
        .context("failed to watch the configuration path")?;

    Ok(Some(watcher))
}

/// Queries the initial workspace and starts the Hyprland event listener.
fn setup_hyprland(runtime: &Rc<RefCell<Runtime>>, bar: &mut BarShell, sender: &Sender<Msg>) {
    let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") else {
        warn!("XDG_RUNTIME_DIR is not set; compositor integration disabled");
        return;
    };
    let Ok(signature) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") else {
        warn!("HYPRLAND_INSTANCE_SIGNATURE is not set; compositor integration disabled");
        return;
    };

    let runtime_dir = PathBuf::from(runtime_dir);

    match active_workspace(&runtime_dir, &signature) {
        Ok(info) => {
            runtime.borrow_mut().dispatch(
                Msg::Compositor(HyprEvent::Workspace { name: info.name }),
                bar,
            );
        }
        Err(error) => warn!(%error, "failed to query the active workspace"),
    }

    let sender = sender.clone();
    match spawn_event_listener(event_socket_path(&runtime_dir, &signature), move |event| {
        sender.send(Msg::Compositor(event)).is_ok()
    }) {
        Ok(_handle) => info!("listening to Hyprland events"),
        Err(error) => warn!(%error, "failed to start the Hyprland event listener"),
    }
}

/// Creates the wgpu instance, preferring Vulkan.
///
/// `Backends::all()` would also initialize the GL driver, which maps over
/// 100 MiB of extra memory on Mesa; GL is only used as a fallback when no
/// Vulkan adapter exists.
fn create_instance() -> wgpu::Instance {
    let vulkan = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let adapters = pollster::block_on(vulkan.enumerate_adapters(wgpu::Backends::VULKAN));
    if adapters.is_empty() {
        warn!("no Vulkan adapter found; falling back to the GL backend");
        return wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
    }

    vulkan
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

/// Executes one kernel effect (the runtime half of effects-as-data).
fn log_effect(level: LogLevel, message: &str) {
    match level {
        LogLevel::Debug => debug!(target: "rusty_skews::kernel", "{message}"),
        LogLevel::Info => info!(target: "rusty_skews::kernel", "{message}"),
        LogLevel::Warn => warn!(target: "rusty_skews::kernel", "{message}"),
        LogLevel::Error => error!(target: "rusty_skews::kernel", "{message}"),
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
