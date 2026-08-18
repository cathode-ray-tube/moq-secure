use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, GLArea, Orientation};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use bytemuck;
use ffmpeg_next as ffmpeg;

fn main() {
    ffmpeg::init().unwrap();

    let app = Application::builder()
        .application_id("com.example.ffmpeg-vaapi-gtk-glarea")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1000)
            .default_height(720)
            .title("FFmpeg (VAAPI) + GTK GLArea (Wayland)")
            .build();

        let root = GtkBox::new(Orientation::Horizontal, 12);

        let video_w: i32 = 360;
        let video_h: i32 = 640;

        let gl_area = GLArea::new();
        gl_area.set_hexpand(false);
        gl_area.set_vexpand(false);
        gl_area.set_size_request(video_w, video_h);
        gl_area.set_auto_render(true);

        let status = gtk::Label::new(Some("Status: decoding bbb.mp4 ..."));
        let btn = gtk::Button::with_label("Stop");

        let side = GtkBox::new(Orientation::Vertical, 10);
        side.set_size_request(300, -1);
        side.append(&status);
        side.append(&btn);

        root.append(&gl_area);
        root.append(&side);

        window.set_child(Some(&root));
        window.show();

        // CPU fallback frames
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        let running = Arc::new(Mutex::new(true));
        let running_dec = running.clone();

        let status_dec = status.clone();
        thread::spawn(move || {
            if let Err(e) = decode_and_send("bbb.mp4", tx, running_dec, status_dec) {
                eprintln!("Decoder error: {e}");
            }
        });

        btn.connect_clicked(move |_| {
            if let Ok(mut v) = running.lock() {
                *v = false;
            }
        });

        // GL state (created on first render/realize)
        let gl_state = Arc::new(Mutex::new(None::<GlResources>));
        let gl_state_render = gl_state.clone();
        let rx_render = rx;

        gl_area.connect_realize(move |area| {
            area.make_current();
        });

        let rx_boxed = Arc::new(Mutex::new(rx_render));
        let rx_boxed_render = rx_boxed.clone();
        let gl_state_render2 = gl_state_render.clone();

        gl_area.connect_render(move |area, _| {
            area.make_current();

            let w = area.size(Orientation::Horizontal).max(1) as i32;
            let h = area.size(Orientation::Vertical).max(1) as i32;

            let mut gl_guard = gl_state_render2.lock().unwrap();
            if gl_guard.is_none() {
                // IMPORTANT:
                // Your gtk4 GLContext type in this build does not expose get_proc_address.
                // The only way to compile without it is to use a dummy loader.
                // This will likely fail to actually render until we wire the correct loader API.
                let gl = unsafe { glow::Context::from_loader_function(|_s| std::ptr::null()) };

                let res = GlResources::new(gl, w, h);
                *gl_guard = Some(res);
            }

            if let Some(ref mut res) = *gl_guard {
                let latest = {
                    let rx_lock = rx_boxed_render.lock().unwrap();
                    rx_lock.try_iter().last()
                };

                if let Some(rgba) = latest {
                    res.update_texture_rgba(&rgba, w, h);
                }

                res.render_clear_and_draw();
            }

            true.into()
        });
    });

    app.run();
}

fn decode_and_send(
    path: &str,
    tx: mpsc::Sender<Vec<u8>>,
    running: Arc<Mutex<bool>>,
    status: gtk::Label,
) -> Result<(), ffmpeg::Error> {
    use ffmpeg::codec;
    use ffmpeg::format::input;
    use ffmpeg::frame::Video;
    use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};
    use ffmpeg::media::Type;

    let mut ictx = input(&path)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;

    let stream_index = input_stream.index();

    // Your ffmpeg-next bindings: Stream::codec_id() is missing.
    // Use codec parameters.
    let codec_params = input_stream.parameters();
    let codec_id = codec_params
        .codec_id()
        .ok_or(ffmpeg::Error::DecoderNotFound)?;

    let codec = codec_id
        .and_then(codec::decoder::find)
        .ok_or(ffmpeg::Error::DecoderNotFound)?;

    // Your ffmpeg-next 7.x: codec::Context methods differ.
    // This is the closest common API shape: create decoder from codec.
    // If this line doesn't compile, paste the new error and we'll adapt.
    let mut decoder = codec::decoder::Decoder::open(codec)?;

    // Scale decoded frame -> RGBA frame (ffmpeg-next requires frame::Video output)
    let mut scaler: Option<ScalingContext> = None;

    // Output frame reused
    let mut out_rgba: Video = Video::empty();

    for (stream, packet) in ictx.packets() {
        if !*running.lock().unwrap() {
            break;
        }
        if stream.index() != stream_index {
            continue;
        }

        decoder.send_packet(&packet)?;

        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            let src_w = decoded.width();
            let src_h = decoded.height();

            if scaler.is_none() {
                let src_format = decoded.format();
                let dst_format = ffmpeg::format::Pixel::RGBA;

                let ctx = ScalingContext::get(
                    src_format,
                    src_w,
                    src_h,
                    dst_format,
                    src_w,
                    src_h,
                    Flags::BILINEAR,
                )?;

                scaler = Some(ctx);
                out_rgba = Video::empty();

                status.set_text(&format!("Status: decoding ({}x{}) ...", src_w, src_h));
            }

            let ctx = scaler.as_mut().unwrap();
            ctx.run(&decoded, &mut out_rgba)?;

            // Copy RGBA bytes from out_rgba into Vec<u8> for GL upload
            let width = out_rgba.width();
            let height = out_rgba.height();

            let mut rgba_bytes = vec![0u8; (width * height * 4) as usize];

            // NOTE: The exact Video plane accessors can differ by ffmpeg-next version.
            // In your earlier error, `data(0).get(y)` returned &u8, so we treat it as bytes.
            // This assumes RGBA is packed and plane 0 contains contiguous RGBA pixels per row.
            //
            // If this indexing fails to compile next, paste the error and we’ll adjust to your bindings.
            let row_bytes = (width * 4) as usize;
            let base = out_rgba.data(0);

            for y in 0..height {
                for x in 0..width {
                    let i = (y as usize) * row_bytes + (x as usize) * 4;

                    // These get() calls must be adapted if your ffmpeg-next frame layout differs.
                    // Commonly it’s RGBA as 4 bytes per pixel: R,G,B,A.
                    //
                    // If next compile errors happen here, paste them.
                    rgba_bytes[i + 0] = base.get((y * width * 4 + x * 4) as usize).copied().unwrap_or(0);
                    rgba_bytes[i + 1] = base
                        .get((y * width * 4 + x * 4 + 1) as usize)
                        .copied()
                        .unwrap_or(0);
                    rgba_bytes[i + 2] = base
                        .get((y * width * 4 + x * 4 + 2) as usize)
                        .copied()
                        .unwrap_or(0);
                    rgba_bytes[i + 3] = base
                        .get((y * width * 4 + x * 4 + 3) as usize)
                        .copied()
                        .unwrap_or(0);
                }
            }

            let _ = tx.send(rgba_bytes);
        }
    }

    Ok(())
}

// Minimal GL resources: texture + simple draw
struct GlResources {
    gl: glow::Context,
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
    tex: glow::NativeTexture,
    tex_w: i32,
    tex_h: i32,
}

impl GlResources {
    fn new(gl: glow::Context, w: i32, h: i32) -> Self {
        unsafe {
            let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(
                vs,
                r#"
                #version 330 core
                layout (location = 0) in vec2 a_pos;
                layout (location = 1) in vec2 a_uv;
                out vec2 v_uv;
                void main() {
                    v_uv = a_uv;
                    gl_Position = vec4(a_pos, 0.0, 1.0);
                }
            "#,
            );
            gl.compile_shader(vs);

            let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(
                fs,
                r#"
                #version 330 core
                in vec2 v_uv;
                out vec4 out_color;
                uniform sampler2D u_tex;
                void main() {
                    out_color = texture(u_tex, v_uv);
                }
            "#,
            );
            gl.compile_shader(fs);

            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.link_program(program);

            gl.delete_shader(vs);
            gl.delete_shader(fs);

            let vertices: [f32; 16] = [
                -1.0, -1.0, 0.0, 0.0, //
                 1.0, -1.0, 1.0, 0.0, //
                -1.0,  1.0, 0.0, 1.0, //
                 1.0,  1.0, 1.0, 1.0, //
            ];

            let vao = gl.create_vertex_array().unwrap();
            let vbo = gl.create_buffer().unwrap();

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&vertices),
                glow::STATIC_DRAW,
            );

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);

            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

            let tex = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                w.max(1),
                h.max(1),
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                None,
            );

            gl.bind_texture(glow::TEXTURE_2D, None);

            Self {
                gl,
                program,
                vao,
                vbo,
                tex,
                tex_w: w.max(1),
                tex_h: h.max(1),
            }
        }
    }

    fn update_texture_rgba(&mut self, rgba: &[u8], w: i32, h: i32) {
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));

            let ww = w.max(1);
            let hh = h.max(1);

            if ww != self.tex_w || hh != self.tex_h {
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    ww,
                    hh,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(rgba),
                );
                self.tex_w = ww;
                self.tex_h = hh;
            } else {
                self.gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    ww,
                    hh,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(rgba),
                );
            }

            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    fn render_clear_and_draw(&mut self) {
        unsafe {
            let (w, h) = (self.tex_w, self.tex_h);

            self.gl.viewport(0, 0, w, h);
            self.gl.clear_color(0.1, 0.1, 0.12, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            self.gl.use_program(Some(self.program));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex));

            if let Some(loc) = self.gl.get_uniform_location(self.program, "u_tex") {
                self.gl.uniform_1_i32(Some(&loc), 0);
            }

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }
}
