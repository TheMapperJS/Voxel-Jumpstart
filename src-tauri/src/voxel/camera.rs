use bevy::prelude::*;
use bevy::input::mouse::MouseMotion;
use bevy::window::{PrimaryWindow, CursorGrabMode, CursorOptions};
use noise::{NoiseFn, Perlin};

#[derive(Component)]
pub struct PlayerController {
    pub speed: f32,
    pub sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub jumping: bool,
}

impl Default for PlayerController {
    fn default() -> Self {
        Self {
            speed: 15.0,
            sensitivity: 0.002,
            yaw: 0.0,
            pitch: 0.0,
            velocity: Vec3::ZERO,
            jumping: false,
        }
    }
}

pub fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut query: Query<(&mut Transform, &mut PlayerController)>,
    mut q_cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    let perlin = Perlin::new(12345);
    let dt = time.delta_secs();

    let cursor = &mut **q_cursor;

    // --- 1. Cursor Management ---
    if mouse_buttons.just_pressed(MouseButton::Left) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }

    // Only move if cursor is locked
    if cursor.grab_mode != CursorGrabMode::Locked {
        for _ in mouse_motion.read() {}
        return;
    }

    for (mut transform, mut player) in &mut query {
        // --- 2. Rotation (Mouse Look) ---
        for event in mouse_motion.read() {
            player.yaw -= event.delta.x * player.sensitivity;
            player.pitch -= event.delta.y * player.sensitivity;
            player.pitch = player.pitch.clamp(-1.5, 1.5);
        }
        
        transform.rotation = Quat::from_axis_angle(Vec3::Y, player.yaw)
            * Quat::from_axis_angle(Vec3::X, player.pitch);
        
        // --- 3. Horizontal Movement (WASD) ---
        let mut move_dir = Vec3::ZERO;
        let forward = transform.forward();
        let right = transform.right();
        
        let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_xz = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        if keys.pressed(KeyCode::KeyW) { move_dir += forward_xz; }
        if keys.pressed(KeyCode::KeyS) { move_dir -= forward_xz; }
        if keys.pressed(KeyCode::KeyA) { move_dir -= right_xz; }
        if keys.pressed(KeyCode::KeyD) { move_dir += right_xz; }

        let horizontal_vel = move_dir.normalize_or_zero() * player.speed;
        
        // --- 4. Vertical Movement (Gravity & Jump) ---
        player.velocity.y -= 30.0 * dt; 

        if keys.just_pressed(KeyCode::Space) && !player.jumping {
            player.velocity.y = 12.0; 
            player.jumping = true;
        }

        transform.translation.x += horizontal_vel.x * dt;
        transform.translation.z += horizontal_vel.z * dt;
        transform.translation.y += player.velocity.y * dt;

        // --- 5. Ground Constraint ---
        let world_x = transform.translation.x as f64;
        let world_z = transform.translation.z as f64;
        let noise_val = perlin.get([world_x * 0.01, world_z * 0.01]);
        let ground_height = (noise_val * 20.0 + 10.0) as f32 + 1.8;

        if transform.translation.y < ground_height {
            transform.translation.y = ground_height;
            player.velocity.y = 0.0;
            player.jumping = false;
        }
    }
}
