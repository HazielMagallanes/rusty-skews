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
use skews_core::{Action, Effect, Effects, InteractionKind, LogLevel, ModuleId};
use skews_ipc_hyprland::{HyprEvent, active_workspace, event_socket_path, spawn_event_listener};
use skews_layout::{BarLayoutOptions, TextMetrics};
use skews_render::{GpuSurface, Renderer};
use skews_text::TextEngine;
use skews_theme::{Tokens, default_palette_path};
use skews_ui::{DrawItem, Scene};
use skews_wayland::{
    BarEvent, BarOptions, BarShell, Connection, EventQueue, Keysym, ObjectId, PANEL_GAP,
    PanelOptions, PointerKind, QueueHandle, display_handle, registry_queue_init, window_handle,
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

    /// Development helper: open a module's panel after the first frame.
    #[arg(long, value_name = "MODULE", hide = true)]
    show_panel: Option<String>,
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

/// Volume panel dimensions.
const PANEL_WIDTH: u32 = 280;
const PANEL_HEIGHT: u32 = 104;
/// Distance from the right edge for dropdown panels.
const PANEL_MARGIN_RIGHT: i32 = 12;
/// Panel corner radius.
const PANEL_RADIUS: f32 = 12.0;
/// Slider geometry inside the panel.
const SLIDER_X: f32 = 18.0;
const SLIDER_Y: f32 = 62.0;
const SLIDER_HEIGHT: f32 = 6.0;
/// Extra vertical hit area around the slider.
const SLIDER_HIT_PADDING: f32 = 10.0;

/// Per-surface GPU state.
struct SurfaceState {
    gpu: GpuSurface,
    width: u32,
    height: u32,
    /// Size the swapchain was last configured for.
    configured: Option<(u32, u32)>,
}

/// A clickable region of the bar, in surface-local pixels.
struct HitRegion {
    module: ModuleId,
    x: f32,
    width: f32,
}

/// A rectangular interactive region inside a panel.
#[derive(Debug, Clone, Copy)]
struct RectRegion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl RectRegion {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// What a mapped surface is used for.
#[derive(Debug, Clone)]
enum SurfaceKind {
    /// A bar strip on an output.
    Bar,
    /// A dropdown panel belonging to a module.
    Panel {
        module: ModuleId,
        backdrop: ObjectId,
    },
    /// The click-catching backdrop of a panel.
    Backdrop { panel: ObjectId },
}

/// Runtime state shared by the calloop sources.
struct Runtime {
    shell: Shell,
    theme: Tokens,
    text_engine: TextEngine,
    instance: wgpu::Instance,
    renderer: Option<Renderer>,
    qh: QueueHandle<BarShell>,
    surfaces: HashMap<ObjectId, SurfaceState>,
    kinds: HashMap<ObjectId, SurfaceKind>,
    hits: HashMap<ObjectId, Vec<HitRegion>>,
    panel_regions: HashMap<ModuleId, Vec<RectRegion>>,
    panels: HashMap<ObjectId, ModuleId>,
    last_output: Option<String>,
    pending_interaction: Option<(ModuleId, InteractionKind)>,
    actions_executed: bool,
}

impl Runtime {
    fn new(shell: Shell, theme: Tokens, qh: QueueHandle<BarShell>) -> Self {
        let text_engine = TextEngine::new(theme.font_family.clone());
        let instance = create_instance();

        Self {
            shell,
            theme,
            text_engine,
            instance,
            renderer: None,
            qh,
            surfaces: HashMap::new(),
            kinds: HashMap::new(),
            hits: HashMap::new(),
            panel_regions: HashMap::new(),
            panels: HashMap::new(),
            last_output: None,
            pending_interaction: None,
            actions_executed: false,
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

        let mut redraw = self.apply(effects, bar);

        // Actions change state outside the kernel (volume, mute, panels);
        // refresh the modules right away instead of waiting for the next tick.
        if self.actions_executed {
            self.actions_executed = false;
            let refresh = self.shell.update(Msg::Tick { unix_ms: unix_ms() });
            redraw |= self.apply(refresh, bar);
        }

        if redraw {
            self.render_all();
        }
    }

    /// Executes effects; returns `true` when a redraw is needed.
    fn apply(&mut self, effects: Effects, bar: &mut BarShell) -> bool {
        let mut redraw = false;

        for effect in effects {
            match effect {
                Effect::Log { level, message } => log_effect(level, &message),
                Effect::Redraw => redraw = true,
                Effect::Action(action) => {
                    self.execute(action, bar);
                    self.actions_executed = true;
                }
            }
        }

        redraw
    }

    fn execute(&mut self, action: Action, bar: &mut BarShell) {
        match action {
            Action::AdjustVolume(delta) => self.audio("adjust volume", |_| {
                skews_services::audio::adjust_volume(delta)
            }),
            Action::SetVolume(volume) => {
                self.audio("set volume", |_| skews_services::audio::set_volume(volume))
            }
            Action::ToggleMute => {
                self.audio("toggle mute", |_| skews_services::audio::toggle_mute());
            }
            Action::TogglePanel(module) => {
                let output = self.last_output.clone();
                self.toggle_panel(&module, output.as_deref(), bar);
            }
        }
    }

    fn audio(
        &self,
        what: &str,
        run: impl FnOnce(()) -> Result<(), skews_services::audio::AudioError>,
    ) {
        if let Err(error) = run(()) {
            warn!(%error, action = what, "audio action failed");
        }
    }

    /// Shows or hides the panel of a module.
    fn toggle_panel(&mut self, module: &ModuleId, output: Option<&str>, bar: &mut BarShell) {
        // Toggle off when this module's panel is open.
        if let Some((id, _)) = self.panels.iter().find(|(_, open)| *open == module) {
            let id = id.clone();
            bar.hide_panel(&id);
            return;
        }

        // Single-panel policy: close anything else first.
        let open: Vec<ObjectId> = self.panels.keys().cloned().collect();
        for id in open {
            bar.hide_panel(&id);
        }

        let options = PanelOptions {
            width: PANEL_WIDTH,
            height: PANEL_HEIGHT,
            margin_top: self.shell.state().bar.height as i32 + PANEL_GAP,
            margin_right: PANEL_MARGIN_RIGHT,
            namespace: format!("rusty-skews-panel-{module}"),
        };

        match bar.show_panel(&self.qh, output, &options) {
            Some((id, backdrop)) => {
                self.kinds.insert(
                    id.clone(),
                    SurfaceKind::Panel {
                        module: module.clone(),
                        backdrop: backdrop.clone(),
                    },
                );
                self.kinds
                    .insert(backdrop, SurfaceKind::Backdrop { panel: id.clone() });
                self.panels.insert(id, module.clone());
            }
            None => warn!(module = %module, "no output available for the panel"),
        }
    }

    fn hide_panels(&mut self, bar: &mut BarShell) {
        let open: Vec<ObjectId> = self.panels.keys().cloned().collect();
        for id in open {
            bar.hide_panel(&id);
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
                    self.close_surface(&id);
                }
                BarEvent::Pointer { id, position, kind } => {
                    self.on_pointer(bar, &id, position, kind);
                }
                BarEvent::PanelShown { id, output, .. } => {
                    info!(panel = ?id, ?output, "panel shown");
                }
                BarEvent::PanelHidden { id } => {
                    info!(panel = ?id, "panel hidden");
                    self.close_surface(&id);
                }
                BarEvent::KeyPressed { keysym } => {
                    if keysym == Keysym::Escape.raw() {
                        self.hide_panels(bar);
                    }
                }
            }
        }
    }

    /// Drops every runtime trace of a closed surface.
    fn close_surface(&mut self, id: &ObjectId) {
        self.surfaces.remove(id);
        self.hits.remove(id);

        match self.kinds.remove(id) {
            Some(SurfaceKind::Panel { module, backdrop }) => {
                self.panel_regions.remove(&module);
                self.panels.remove(id);
                self.kinds.remove(&backdrop);
            }
            Some(SurfaceKind::Backdrop { panel }) => {
                self.panels.remove(&panel);
                self.kinds.remove(&panel);
            }
            _ => {}
        }
    }

    /// Routes pointer input to the module or panel under the cursor.
    fn on_pointer(
        &mut self,
        bar: &mut BarShell,
        id: &ObjectId,
        position: (f64, f64),
        kind: PointerKind,
    ) {
        let x = position.0 as f32;
        let y = position.1 as f32;

        match self.kinds.get(id).cloned() {
            Some(SurfaceKind::Backdrop { panel }) => {
                // Any press on the backdrop dismisses the panel.
                if matches!(kind, PointerKind::Press { .. }) {
                    bar.hide_panel(&panel);
                }
                return;
            }
            Some(SurfaceKind::Panel { module, .. }) => {
                self.on_panel_pointer(bar, &module, x, y, kind);
                return;
            }
            _ => {}
        }

        let Some(module) = self.hits.get(id).and_then(|hits| {
            hits.iter()
                .find(|hit| x >= hit.x && x < hit.x + hit.width)
                .map(|hit| hit.module.clone())
        }) else {
            return;
        };

        let interaction = interaction_from_pointer(kind);
        if let Some(kind) = interaction {
            self.last_output = bar.surface_output(id).or(self.last_output.clone());
            debug!(module = %module, ?kind, "module interaction");
            self.dispatch(Msg::Interaction { module, kind }, bar);
        }
    }

    /// Handles clicks and scrolls inside a panel.
    fn on_panel_pointer(
        &mut self,
        bar: &mut BarShell,
        module: &ModuleId,
        x: f32,
        y: f32,
        kind: PointerKind,
    ) {
        match kind {
            PointerKind::Press { button: 0x110 } => {
                let Some(regions) = self.panel_regions.get(module).cloned() else {
                    return;
                };
                let Some(region) = regions.iter().find(|region| region.contains(x, y)) else {
                    return;
                };

                let fraction = ((x - region.x) / region.width).clamp(0.0, 1.0);
                self.execute(Action::SetVolume(fraction), bar);
                self.refresh_after_action(bar);
            }
            PointerKind::Scroll { vertical, .. } if vertical != 0.0 => {
                let interaction = if vertical > 0.0 {
                    InteractionKind::ScrollUp
                } else {
                    InteractionKind::ScrollDown
                };
                self.dispatch(
                    Msg::Interaction {
                        module: module.clone(),
                        kind: interaction,
                    },
                    bar,
                );
            }
            _ => {}
        }
    }

    /// Refreshes modules and re-renders after an out-of-band change.
    fn refresh_after_action(&mut self, bar: &mut BarShell) {
        let refresh = self.shell.update(Msg::Tick { unix_ms: unix_ms() });
        let _ = self.apply(refresh, bar);
        self.render_all();
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
            .context("failed to render the surface")?;

        match self.kinds.get(id).cloned() {
            Some(SurfaceKind::Panel { module, .. }) => {
                info!(panel = ?id, module = %module, width, height, "panel configured");
            }
            _ => {
                self.kinds.entry(id.clone()).or_insert(SurfaceKind::Bar);
                info!(surface = ?id, width, height, "bar configured");
                let plan = self.shell.bar_plan();
                info!(
                    left = %describe(&plan.left),
                    center = %describe(&plan.center),
                    right = %describe(&plan.right),
                    "bar plan"
                );

                // Development helper: fire one interaction after the first frame.
                if let Some((module, kind)) = self.pending_interaction.take() {
                    info!(module = %module, ?kind, "dispatching pending interaction");
                    self.dispatch(Msg::Interaction { module, kind }, bar);
                }
            }
        }

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
        let Some(state) = self.surfaces.get(id) else {
            return Ok(());
        };
        let (width, height) = (state.width, state.height);
        let kind = self.kinds.get(id).cloned().unwrap_or(SurfaceKind::Bar);
        let is_bar = matches!(kind, SurfaceKind::Bar);

        // Build the scene first: it may borrow the text engine mutably.
        let (scene, hits, panel_regions) = match kind {
            SurfaceKind::Panel { module, .. } => {
                let (scene, regions) = self.build_panel_scene(&module, width, height);
                (scene, Vec::new(), Some((module, regions)))
            }
            SurfaceKind::Backdrop { .. } => (
                Scene {
                    background: skews_core::Rgba::TRANSPARENT,
                    items: Vec::new(),
                },
                Vec::new(),
                None,
            ),
            SurfaceKind::Bar => {
                let (scene, hits) = build_scene(
                    &self.shell,
                    &mut self.text_engine,
                    &self.theme,
                    width,
                    height,
                )?;
                (scene, hits, None)
            }
        };

        if let Some((module, regions)) = panel_regions {
            self.panel_regions.insert(module, regions);
        }
        if is_bar {
            self.hits.insert(id.clone(), hits);
        }

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

        renderer
            .render_scene(&mut state.gpu, &scene, &mut self.text_engine)
            .context("failed to render the scene")?;

        Ok(())
    }

    /// Builds the scene and interactive regions of a module's panel.
    fn build_panel_scene(
        &mut self,
        module: &ModuleId,
        width: u32,
        height: u32,
    ) -> (Scene, Vec<RectRegion>) {
        const TITLE_SIZE: f32 = 11.0;
        const VALUE_SIZE: f32 = 13.0;

        let level = self.module_level(module);
        let (percent, muted) = level.unwrap_or((0.0, false));

        let track_width = width as f32 - SLIDER_X * 2.0;
        let fill_width = track_width * (percent / 100.0).clamp(0.0, 1.0);

        let value_text = if muted {
            String::from("MUTED")
        } else {
            format!("{}%", percent.round() as i32)
        };
        let value_width = self.text_engine.measure(&value_text, VALUE_SIZE).width;

        let accent = if muted {
            self.theme.error
        } else {
            self.theme.primary
        };

        let scene = Scene {
            background: skews_core::Rgba::TRANSPARENT,
            items: vec![
                DrawItem::Rect {
                    rect: skews_core::Rect::new(0, 0, width, height),
                    color: self.theme.surface_container.with_alpha(0.96),
                    radius: PANEL_RADIUS,
                },
                DrawItem::Text {
                    text: String::from("Volume"),
                    x: SLIDER_X as i32,
                    y: 14,
                    size: TITLE_SIZE,
                    color: self.theme.on_surface.with_alpha(0.7),
                },
                DrawItem::Text {
                    text: value_text,
                    x: (width as f32 - SLIDER_X - value_width).round() as i32,
                    y: 12,
                    size: VALUE_SIZE,
                    color: self.theme.on_surface,
                },
                DrawItem::Rect {
                    rect: skews_core::Rect::new(
                        SLIDER_X as i32,
                        SLIDER_Y as i32,
                        track_width.round() as u32,
                        SLIDER_HEIGHT as u32,
                    ),
                    color: self.theme.on_surface.with_alpha(0.25),
                    radius: SLIDER_HEIGHT / 2.0,
                },
                DrawItem::Rect {
                    rect: skews_core::Rect::new(
                        SLIDER_X as i32,
                        SLIDER_Y as i32,
                        fill_width.round() as u32,
                        SLIDER_HEIGHT as u32,
                    ),
                    color: accent,
                    radius: SLIDER_HEIGHT / 2.0,
                },
            ],
        };

        let regions = vec![RectRegion {
            x: SLIDER_X,
            y: SLIDER_Y - SLIDER_HIT_PADDING,
            width: track_width,
            height: SLIDER_HEIGHT + SLIDER_HIT_PADDING * 2.0,
        }];

        (scene, regions)
    }

    /// Returns a module's level output, when it publishes one.
    fn module_level(&self, module: &ModuleId) -> Option<(f32, bool)> {
        let plan = self.shell.bar_plan();
        let slots = [plan.left, plan.center, plan.right].concat();

        slots.iter().find_map(|slot| {
            if &slot.id != module {
                return None;
            }
            match &slot.output {
                ModuleOutput::Level { percent, muted } => Some((*percent, *muted)),
                _ => None,
            }
        })
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
    let runtime = Rc::new(RefCell::new(Runtime::new(shell, theme, qh.clone())));

    if let Some(module) = &args.show_panel {
        runtime.borrow_mut().pending_interaction =
            Some((ModuleId::from(module.as_str()), InteractionKind::Click));
    }

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

/// Maps a pointer event to a module interaction, when it is one.
fn interaction_from_pointer(kind: PointerKind) -> Option<InteractionKind> {
    match kind {
        // Linux button codes: 0x110 = left, 0x111 = right.
        PointerKind::Press { button: 0x110 } => Some(InteractionKind::Click),
        PointerKind::Press { button: 0x111 } => Some(InteractionKind::SecondaryClick),
        PointerKind::Scroll { vertical, .. } if vertical > 0.0 => Some(InteractionKind::ScrollUp),
        PointerKind::Scroll { vertical, .. } if vertical < 0.0 => Some(InteractionKind::ScrollDown),
        _ => None,
    }
}

/// Builds the bar scene from the kernel plan, the layout pass and the theme.
fn build_scene(
    shell: &Shell,
    text_engine: &mut TextEngine,
    theme: &Tokens,
    width: u32,
    height: u32,
) -> Result<(skews_ui::Scene, Vec<HitRegion>)> {
    const TEXT_SIZE: f32 = 12.0;
    const HIT_PADDING: f32 = 4.0;

    let plan = shell.bar_plan();
    let mut measure = |slots: &[ModuleSlot]| -> Vec<(ModuleId, String, TextMetrics)> {
        slots
            .iter()
            .filter_map(|slot| match &slot.output {
                ModuleOutput::Text(text) => {
                    let shaped = text_engine.measure(text, TEXT_SIZE);
                    Some((
                        slot.id.clone(),
                        text.clone(),
                        TextMetrics {
                            width: shaped.width,
                            height: shaped.height,
                        },
                    ))
                }
                ModuleOutput::Level { percent, muted } => {
                    let text = level_text(*percent, *muted);
                    let shaped = text_engine.measure(&text, TEXT_SIZE);
                    Some((
                        slot.id.clone(),
                        text,
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

    let metrics = |items: &[(ModuleId, String, TextMetrics)]| {
        items
            .iter()
            .map(|(_, _, metrics)| *metrics)
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
    let mut hits = Vec::new();
    for (positions, items) in [
        (&layout.left, &left),
        (&layout.center, &center),
        (&layout.right, &right),
    ] {
        for (position, (module, text, item_metrics)) in positions.iter().zip(items.iter()) {
            texts.push(skews_ui::BarText {
                text: text.clone(),
                x: position.x,
                y: position.y,
                size: TEXT_SIZE,
                color: theme.on_surface,
            });
            hits.push(HitRegion {
                module: module.clone(),
                x: position.x - HIT_PADDING,
                width: item_metrics.width + HIT_PADDING * 2.0,
            });
        }
    }

    Ok((
        skews_ui::build_bar_scene(theme.surface_container.with_alpha(0.9), &texts),
        hits,
    ))
}

/// Renders a level output as bar text.
fn level_text(percent: f32, muted: bool) -> String {
    if muted {
        String::from("MUTED")
    } else {
        format!("{}%", percent.round() as i32)
    }
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
            ModuleOutput::Level { percent, muted } => {
                format!("{}={}", slot.id, level_text(*percent, *muted))
            }
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
