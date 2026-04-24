pub mod voxel;

use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::window::{PrimaryWindow, RawHandleWrapper, WindowWrapper};
use bevy::winit::WinitPlugin;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use tauri::Manager;
use voxel::{VoxelMaterial, VoxelPlugin};
use voxel::material::VoxelMaterialExtension;
use voxel::world::VoxelAssets;
use bevy::pbr::ExtendedMaterial;
use std::sync::Mutex;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Represents input captured from the React frontend and sent to Bevy.
/// We use camelCase renaming to match standard JavaScript event objects.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TauriInput {
    KeyDown(String),
    KeyUp(String),
    MouseMove { dx: f32, dy: f32 },
    MouseDown(u32),
    MouseUp(u32),
}

/// A thread-safe wrapper for the input receiver.
/// Bevy systems will poll this to sync the engine's state with the UI.
#[derive(Resource)]
struct BevyMessageReceiver(pub Mutex<Receiver<TauriInput>>);

// Stores window handles and dimensions required to bridge Tauri and Bevy.
#[derive(Resource, Clone)]
struct BevyWindowHandle {
    window_handle: RawWindowHandle,
    display_handle: RawDisplayHandle,
    physical_size: Vec2,
    scale_factor: f32,
    app_handle: tauri::AppHandle,
}

impl HasWindowHandle for BevyWindowHandle {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        unsafe { Ok(raw_window_handle::WindowHandle::borrow_raw(self.window_handle)) }
    }
}

impl HasDisplayHandle for BevyWindowHandle {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        unsafe { Ok(raw_window_handle::DisplayHandle::borrow_raw(self.display_handle)) }
    }
}

// These are required to move the handle resource into the Bevy thread.
unsafe impl Send for BevyWindowHandle {}
unsafe impl Sync for BevyWindowHandle {}

/// Tauri command called from React to forward input events into the Bevy channel.
#[tauri::command]
fn send_input(input: TauriInput, sender: tauri::State<Sender<TauriInput>>) {
    let _ = sender.send(input);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // We use a cross-thread channel to bridge the Tauri (UI) thread and the Bevy (Engine) thread.
    let (tx, rx) = channel::<TauriInput>();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(tx) // Make the sender available to Tauri commands
        .setup(move |app| {
            let main_window = app.get_webview_window("main").unwrap();
            let app_handle = app.handle().clone();
            
            // SPAWN BEVY IN A BACKGROUND THREAD
            // This is critical. Bevy and Tauri both want to own the main thread's event loop.
            // By moving Bevy to a thread and using a manual loop, we keep both happy.
            std::thread::spawn(move || {
                // Wait for the Tauri window to stabilize before extracting OS handles.
                std::thread::sleep(std::time::Duration::from_millis(800));

                let handle = {
                    let win_handle = main_window.window_handle();
                    let disp_handle = main_window.display_handle();
                    let size = main_window.inner_size().unwrap();
                    let scale_factor = main_window.scale_factor().unwrap();
                    
                    match (win_handle, disp_handle) {
                        (Ok(w), Ok(d)) => {
                            BevyWindowHandle {
                                window_handle: w.as_raw(),
                                display_handle: d.as_raw(),
                                physical_size: Vec2::new(size.width as f32, size.height as f32),
                                scale_factor: scale_factor as f32,
                                app_handle: app_handle,
                            }
                        }
                        _ => panic!("CRITICAL: Failed to get OS window handles"),
                    }
                };

                let mut app = App::new();
                app.add_plugins(
                    DefaultPlugins
                        .build()
                        .disable::<WinitPlugin>() // Tauri owns the window, not Bevy
                        .disable::<PipelinedRenderingPlugin>() // Required for custom window handles
                        .set(bevy::window::WindowPlugin {
                            primary_window: None, // We will spawn the window manually
                            exit_condition: bevy::window::ExitCondition::DontExit,
                            close_when_requested: false,
                            ..default()
                        }),
                )
                .add_plugins(VoxelPlugin)
                .insert_resource(BevyMessageReceiver(Mutex::new(rx)))
                .insert_resource(GlobalAmbientLight {
                    color: Color::WHITE,
                    brightness: 500.0,
                    affects_lightmapped_meshes: true,
                })
                .insert_resource(ClearColor(Color::NONE)) // Transparent background for the overlay
                .insert_resource(handle)
                .add_systems(Startup, (attach_native_window, setup_bevy_scene).chain())
                .add_systems(Update, (sync_tauri_window_size, handle_tauri_input));

                app.finish();
                app.cleanup();

                // MANUAL ENGINE LOOP
                // We tick Bevy at 60Hz. app.update() processes one frame of ECS systems.
                let mut last_frame = std::time::Instant::now();
                loop {
                    let target_dt = std::time::Duration::from_secs_f64(1.0 / 60.0);
                    let elapsed = last_frame.elapsed();
                    if elapsed < target_dt {
                        std::thread::sleep(target_dt - elapsed);
                    }
                    last_frame = std::time::Instant::now();
                    app.update();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![send_input])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Spawns the Bevy Window entity using the handles extracted from Tauri.
fn attach_native_window(
    mut commands: Commands,
    handle: Res<BevyWindowHandle>,
) {
    // WindowWrapper is the safe bridge for RawHandleWrapper in Bevy 0.18
    let wrapper = RawHandleWrapper::new(&WindowWrapper::new(handle.clone()))
        .expect("Failed to create RawHandleWrapper");

    commands.spawn((
        Window {
            title: "Bevy Overlay".into(),
            transparent: true,
            resolution: bevy::window::WindowResolution::new(handle.physical_size.x as u32, handle.physical_size.y as u32)
                .with_scale_factor_override(handle.scale_factor),
            ..default()
        },
        PrimaryWindow,
        wrapper,
    ));
}

/// Polls the input channel and manually updates Bevy's input resources.
/// This is needed because Bevy's normal input systems are tied to Winit (which we disabled).
fn handle_tauri_input(
    receiver: Res<BevyMessageReceiver>,
    mut keyboard_input: ResMut<ButtonInput<KeyCode>>,
    mut mouse_button_input: ResMut<ButtonInput<MouseButton>>,
    mut mouse_motion_writer: MessageWriter<bevy::input::mouse::MouseMotion>,
) {
    let Ok(rx) = receiver.0.lock() else { return };
    while let Ok(input) = rx.try_recv() {
        match input {
            TauriInput::KeyDown(key) => {
                if let Some(keycode) = parse_key_code(&key) {
                    keyboard_input.press(keycode);
                }
            }
            TauriInput::KeyUp(key) => {
                if let Some(keycode) = parse_key_code(&key) {
                    keyboard_input.release(keycode);
                }
            }
            TauriInput::MouseMove { dx, dy } => {
                mouse_motion_writer.write(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(dx, dy),
                });
            }
            TauriInput::MouseDown(button) => {
                let mouse_button = match button {
                    0 => MouseButton::Left,
                    1 => MouseButton::Middle,
                    2 => MouseButton::Right,
                    _ => continue,
                };
                mouse_button_input.press(mouse_button);
            }
            TauriInput::MouseUp(button) => {
                let mouse_button = match button {
                    0 => MouseButton::Left,
                    1 => MouseButton::Middle,
                    2 => MouseButton::Right,
                    _ => continue,
                };
                mouse_button_input.release(mouse_button);
            }
        }
    }
}

/// Helper to map JavaScript event strings to Bevy KeyCodes.
fn parse_key_code(key: &str) -> Option<KeyCode> {
    match key.to_lowercase().as_str() {
        "w" => Some(KeyCode::KeyW),
        "s" => Some(KeyCode::KeyS),
        "a" => Some(KeyCode::KeyA),
        "d" => Some(KeyCode::KeyD),
        " " | "space" => Some(KeyCode::Space),
        "shift" => Some(KeyCode::ShiftLeft),
        "escape" => Some(KeyCode::Escape),
        _ => None,
    }
}

/// Proactively matches Bevy's resolution/scaling to the Tauri window.
/// This prevents the "stuck in a corner" bug by ensuring physical and logical pixels align.
fn sync_tauri_window_size(
    handle: Res<BevyWindowHandle>,
    mut q_window: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut q_camera: Query<&mut Projection, With<Camera>>,
) {
    let Some(mut window) = q_window.iter_mut().next() else { return };

    if let Some(main_win) = tauri::Manager::get_webview_window(&handle.app_handle, "main") {
        if let Ok(size) = main_win.inner_size() {
            let width = size.width as f32;
            let height = size.height as f32;

            if width > 0.0 && height > 0.0 {
                let tauri_sf = main_win.scale_factor().unwrap_or(1.0) as f32;
                
                // Only update if the dimensions have actually changed
                if (window.physical_width() as f32 - width).abs() > 0.1 
                   || (window.physical_height() as f32 - height).abs() > 0.1 
                   || (window.resolution.scale_factor() - tauri_sf).abs() > 0.01 
                {
                    window.resolution.set_physical_resolution(size.width, size.height);
                    window.resolution.set_scale_factor(tauri_sf);
                    
                    // Force camera to update its aspect ratio immediately
                    for mut projection in q_camera.iter_mut() {
                        if let Projection::Perspective(ref mut p) = *projection {
                            p.aspect_ratio = width / height;
                        }
                    }
                }
            }
        }
    }
}

fn setup_bevy_scene(
    mut commands: Commands,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    // Initialize Voxel Assets using the ExtendedMaterial API
    let material_handle = materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.2, 0.8, 0.3),
            perceptual_roughness: 0.7,
            ..default()
        },
        extension: VoxelMaterialExtension {},
    });

    commands.insert_resource(VoxelAssets {
        material: material_handle,
    });

    // Simple lighting setup
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 15000.0,
            ..default()
        },
        Transform::from_xyz(40.0, 100.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Initialize the First-Person Player
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::from_xyz(0.0, 50.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
        Camera {
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        voxel::camera::PlayerController::default(),
    ));
}
