use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box as GtkBox, GLArea, Orientation};

fn main() {
    let app = Application::builder()
        .application_id("com.example.gst-ffmpeg-gtk-gl-starter")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(900)
            .default_height(700)
            .title("GTK GLArea Video Box (FFmpeg later)")
            .build();

        // Root: left video region (fixed portrait box) + right UI panel
        let root = GtkBox::new(Orientation::Horizontal, 12);

        // Portrait fixed video box
        let video_w: i32 = 360;
        let video_h: i32 = 640;

        // GLArea works on Wayland and X11, and is the right place for low-latency texture rendering.
        let gl_area = GLArea::new();
        gl_area.set_hexpand(false);
        gl_area.set_vexpand(false);
        gl_area.set_size_request(video_w, video_h);

        // Basic GL setup
        gl_area.set_auto_render(true);

        // Placeholder rendering: clear with gradient-ish color + aspect-box
        // (Later you'll replace this draw code with NV12 texture rendering.)
        gl_area.connect_realize(|area| {
            // Create the GL context.
            area.make_current();
            let gl = area.context();
            let _ = gl; // placeholder to ensure context is created
        });

        // In real code, you’d use gl bindings (glow, glutin, etc.) to draw.
        // This starter keeps it minimal: we just force an initial render.
        gl_area.connect_render(|area, _ctx| {
            // Returning true indicates we handled the frame.
            // For now, we just request another render; the GL drawing will come later.
            area.queue_render();
            true.into()
        });

        // Right-side UI panel placeholder
        let side = GtkBox::new(Orientation::Vertical, 10);
        side.set_size_request(260, -1);

        let status = gtk::Label::new(Some("Status: placeholder (wire FFmpeg+VAAPI next)"));
        let btn = gtk::Button::with_label("Example button");

        side.append(&status);
        side.append(&btn);

        root.append(&gl_area);
        root.append(&side);

        window.set_child(Some(&root));
        window.show();
    });

    app.run();
}
