//#![windows_subsystem = "windows"]
#![feature(allocator_api)]

use std::time::Instant;
use std::boxed::Box;
use std::borrow::Cow;

use tev_client::{TevClient, PacketCreateImage, PacketUpdateImage};
use bytemuck::cast_slice;

use math::{
    vec::{Vec2, Vec3, Vec4},
    mat::Mat4,
};
use scene::{Camera, Direction};
use render::{Raster, Ray, Pipeline, SceneConstants};
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

mod win32;
mod d3d12;
mod shaders;
mod imgui_impl;
mod render;

mod allocator;

#[allow(unused_macros)]
macro_rules! debug_break {
    () => {
        unsafe {
            core::arch::asm!("int 3");
        }
    }
}

pub fn screenshot(scene: &mut Box<dyn Pipeline>, d3d12: &d3d12::Context,
    constants: &SceneConstants, width: u32, height: u32) {

    if let Some((name, data)) = scene.capture_screenshot(&d3d12, &constants){
        if let Ok(stream) = std::net::TcpStream::connect("127.0.0.1:14158") {
            let mut client = TevClient::wrap(stream);

            let channel_names = ["R", "G", "B"];
            client.send(PacketCreateImage {
                image_name: &name,
                grab_focus: false,
                width: width,
                height: height,
                channel_names: &channel_names,
            }).unwrap();

            client.send(PacketUpdateImage {
                image_name: &name,
                grab_focus: false,
                channel_names: &channel_names,
                channel_offsets: &[0, 1, 2],
                channel_strides: &[3, 3, 3],
                x: 0,
                y: 0,
                width: width,
                height: height,
                data: cast_slice(&data),
            }).unwrap();
        }
    }
}


fn main() {
    use std::path::Path;

    let path = std::env::args().nth(1).unwrap_or(
        String::from("res/bistro.lz4"));

    let mut scene = asset::load_scene_from_asset_file(&Path::new(&path))
        .expect("Failed to open asset file");

    // Transform to z up;
    let to_z_up = Mat4::from_columns(&[
        Vec4::new(1., 0., 0., 0.),
        Vec4::new(0., 0., 1., 0.),
        Vec4::new(0., 1., 0., 0.),
        Vec4::new(0., 0., 0., 1.),
    ]).transpose();

    for m in scene.meshes.iter_mut() {
        m.transform = to_z_up * m.transform;
    }


    let mut window = win32::create_window("Rust window", 1280, 720)
        .expect("Failed to create window");

    let mut d3d12 = d3d12::Context::init(&window)
        .expect("Failed to initialize D3D12");

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let mut imgui_impl = imgui_impl::Backend::init(&d3d12, &mut imgui,
                                                   window.width(),
                                                   window.height())
        .expect("Failed to initialize imgui backend");


    let raster = Box::new(Raster::init(&window, &d3d12, &scene));
    let ray = Box::new(Ray::init(&window, &d3d12, &scene));

    assert!(d3d12.csu_descriptor_heap.offset() == 10);

    let mut textures: Vec<ID3D12Resource> = Vec::new();
    for img in &scene.images {
        let descriptor = d3d12.alloc_csu_descriptor().unwrap();
        let format = match img.format {
            scene::Format::RGBA8 => d3d12::DXGI_FORMAT_R8G8B8A8_UNORM,
            scene::Format::SRGBA8 => d3d12::DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        };
        let texture = d3d12.upload_tex2d_sync(&img.data, img.width, img.height,
            format, d3d12::D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)
            .expect("Failed to upload texture");
        d3d12.create_shader_resource_view_tex2d(&texture, format, descriptor);
        textures.push(texture);
    }
    d3d12.wait_sync_commands();


    let mut ray_scene:  Box<dyn Pipeline> = ray;
    let mut raster_scene: Box<dyn Pipeline> = raster;

    let mut scene: &mut Box<dyn Pipeline> = &mut ray_scene;

    let camera_pos = Vec3::new(-21., -1.0, 5.5);
    let mut camera = Camera::new(
        camera_pos,
        Vec3::new(0., 0., 0.),
        Vec3::new(0., 0., 1.),
        0., 0., 0., 0., 20., 1.
    );


    let mut constants = SceneConstants {
        camera_position: camera_pos,
        camera_direction: camera.forward,
        light_direction: Vec3::new(-0.496, 0.694, -0.522).normalized(),
        light_radiance: 0.0,
        diffuse_color: Vec3::new(0., 1., 0.),
        film_dist: 0.7,
        emissive_multiplier: 100.0,
        bounces: 8,
        sampling_mode: 3,
        use_alias_table: 1,
        ris_count: 128,
        ..Default::default()
    };
    const NIGHT: bool = false;
    if NIGHT {
        constants.light_radiance = 0.0;
        constants.emissive_multiplier = 100.0;
    }

    let mut timestamp = Instant::now();
    let mut frame_times = [-1.0f64; 64];
    let mut frame_time_index: usize = 0;
    let mut frame_index: u32 = 0;

    let mut dragging = false;
    let mut moving = false;
    let mut direction = Direction::Forward;

    let mut auto_screenshot = false;
    'main: loop {
        let mut reset = false;

        let now = Instant::now();
        let dt = (now - timestamp).as_secs_f64();
        timestamp = now;

        frame_times[frame_time_index] = dt;
        let avg_frame_time = frame_times.iter()
            .fold(0.0, |s, x| if *x > 0.0 { s + x } else { s }) /
            frame_times.iter().fold(0, |s, x| s + ((*x > 0.0) as u32)) as f64;
        frame_time_index = (frame_time_index + 1) % frame_times.len();
        let dt = dt as f32;

        while let Some(event) = window.poll_events() {
            let io = imgui.io_mut();

            use win32::{Event::*, MouseButton};
            match event {
                Quit => break 'main,
                KeyPress(Some(i @ ('1' | '2' | '3' | '4'))) => {
                    let i = i.to_digit(10).unwrap() - 1;
                    constants.sampling_mode = i;
                    reset = true;
                },

                KeyPress(Some('W')) => {
                    direction = Direction::Forward;
                    moving = true;
                }
                KeyPress(Some('A')) => {
                    direction = Direction::Left;
                    moving = true;
                }
                KeyPress(Some('S')) => {
                    direction = Direction::Backward;
                    moving = true;
                }
                KeyPress(Some('D')) => {
                    direction = Direction::Right;
                    moving = true;
                }
                KeyPress(Some('Q')) => {
                    direction = Direction::Down;
                    moving = true;
                }
                KeyPress(Some('E')) => {
                    direction = Direction::Up;
                    moving = true;
                }
                KeyRelease(_) => {
                    moving = false;
                }

                KeyPress(Some('O')) => {
                    reset = true;
                    auto_screenshot = true;
                }

                KeyPress(Some('P')) => {
                    screenshot(scene, &d3d12, &constants,
                               window.width(), window.height());
                }

                KeyPress(Some('R')) => {
                    scene = &mut raster_scene;
                }
                KeyPress(Some('T')) => {
                    scene = &mut ray_scene;
                    reset = true;
                }

                MouseMove(x, y) => {
                    let delta = Vec2::new(
                        x as f32 - io.mouse_pos[0],
                        y as f32 - io.mouse_pos[1]
                    );

                    io.mouse_pos[0] = x as f32;
                    io.mouse_pos[1] = y as f32;

                    if dragging {
                        let offset = delta / Vec2::new(window.width() as f32,
                            window.height() as f32);
                        camera.drag(offset);
                        reset = true;
                    }
                }

                MouseLeave => {
                    io.mouse_pos[0] = f32::MAX;
                    io.mouse_pos[1] = f32::MAX;
                }

                MouseWheel(hor, vert) => {
                    io.mouse_wheel_h = hor;
                    io.mouse_wheel = vert;
                }

                MousePress(MouseButton::Left)   => {
                    io.mouse_down[0] = true;
                    if !io.want_capture_mouse {
                        dragging = true;
                    }
                }

                MousePress(MouseButton::Right)  => io.mouse_down[1] = true,
                MousePress(MouseButton::Middle) => io.mouse_down[2] = true,

                MouseRelease(MouseButton::Left)   => {
                  io.mouse_down[0] = false;
                  dragging = false;
                }

                MouseRelease(MouseButton::Right)  => io.mouse_down[1] = false,
                MouseRelease(MouseButton::Middle) => io.mouse_down[2] = false,

                Focus(in_focus) => io.app_focus_lost = !in_focus,

                Minimized => io.display_size = [0., 0.],
                Resize(width, height) => {
                    io.display_size = [width as f32, height as f32];
                    d3d12.resize(width, height).expect("Failed to resize");
                    scene.resize(&d3d12, width, height);
                },
                _ => {}
            }
        }

        {
            let (frame, index) = d3d12.begin_frame()
                .expect("Failed to begin frame");

            if moving {
                camera.move_in_direction(direction, dt);
                reset = true;
            }

            camera.aspect_ratio =
                window.height() as f32 / window.width() as f32;
            camera.near = 0.1;
            camera.far = 1000.0;
            camera.fov = 2. * (1. / (constants.film_dist * 2.)).atan();


            constants.camera_position = camera.position;
            constants.camera_direction = camera.forward;
            constants.view = camera.view();
            constants.projection = camera.projection();
            constants.frame_index = frame_index;

            imgui_impl.frame(&mut imgui, |ui| {
                // let mut opened = true;
                // ui.show_demo_window(&mut opened);

                ui.window("Renderer")
                    .position([20., 20.], imgui::Condition::FirstUseEver)
                    .size([380., 600.], imgui::Condition::FirstUseEver)
                    .build(|| {

                    ui.text(format!("{:.3}ms ({:.3}fps)",
                                    avg_frame_time * 1000.0,
                                    1.0 / avg_frame_time));
                    ui.separator();
                    ui.text("Scene:");
                    let mut pos = constants.camera_position.to_slice();
                    if imgui::Drag::new("Camera position")
                        .build_array(&ui, &mut pos) {
                        reset = true;
                        camera.position = Vec3::from_slice(&pos);
                    }

                    if imgui::Drag::new("Film distance").range(0.1, 2.0)
                        .speed(0.01).build(&ui, &mut constants.film_dist) {
                        reset = true;
                    }

                    let mut dir = constants.light_direction.to_slice();
                    if imgui::Drag::new("Sun direction")
                        .speed(0.01)
                        .build_array(&ui, &mut dir) {
                        reset = true;
                        constants.light_direction =
                            Vec3::from_slice(&dir).normalized();
                    }

                    if imgui::Drag::new("Sun Radiance")
                        .range(0., 100.0)
                        .speed(0.1)
                        .build(&ui, &mut constants.light_radiance) {
                        reset = true;
                    }

                    if imgui::Drag::new("Emissive")
                        .range(0., 1000.0)
                        .speed(0.1)
                        .build(&ui, &mut constants.emissive_multiplier) {
                        reset = true;
                    }

                    ui.separator();
                    ui.text("Settings:");
                    if ui.input_scalar("Bounces",  &mut constants.bounces)
                        .step(1).build() {
                        reset = true;
                    }

                    let mut use_alias = constants.use_alias_table != 0;
                    if ui.checkbox("Use alias table",  &mut use_alias) {
                        constants.use_alias_table = use_alias.into();
                        reset = true;
                    }

                    let mut index = constants.sampling_mode as usize;
                    let items = [
                        "Lights",
                        "BRDF",
                        "MIS",
                        "RIS",
                    ];
                    if ui.combo("Sampling mode", &mut index,
                        &items, |v| Cow::from(*v)) {

                        reset = true;
                        constants.sampling_mode = index as u32;
                    }

                    ui.input_scalar("RIS count",  &mut constants.ris_count).build();

                    ui.same_line();
                    if ui.button("+") {
                        reset = true;
                        constants.ris_count = (constants.ris_count * 2).min(1024);
                    }
                    ui.same_line();
                    if ui.button("-") {
                        reset = true;
                        constants.ris_count = (constants.ris_count / 2).max(1);
                    }


                    let mut index = constants.debug as usize;
                    let items = [
                        "Rendered",
                        "Roughness",
                        "Metallic",
                        "Emissive",
                        "Normals",
                        "Debug",
                    ];
                    if ui.combo("Debug view", &mut index,
                        &items, |v| Cow::from(*v)) {

                        reset = true;
                        constants.debug = index as u32;
                    }

                    ui.text(format!("Sun p: {}", (constants.light_radiance * 10.0 /
                        (constants.light_radiance * 10.0 + constants.emissive_multiplier)).clamp(0.05, 0.95)));
                    ui.text(format!("Samples: {}", constants.samples));
                });
            });

            scene.render(&d3d12, &frame, index, &mut constants, reset);
            imgui_impl.render(&mut imgui, &d3d12, &frame, index);

            d3d12.end_frame(frame, true).expect("Failed to end frame");

            if auto_screenshot && scene.sample_count().is_power_of_two() {
                screenshot(scene, &d3d12, &constants,
                    window.width(), window.height());
            }

            if scene.sample_count() == render::MAX_SAMPLES {
                auto_screenshot = false;
            }
        }
        frame_index += 1;
    }

    //Wait for the last frame we issued before shutting down
    d3d12.wait_idle();
}
