// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// external
use cairo;
use gdk_pixbuf::{
    self,
    PixbufExt,
    PixbufLoaderExt,
};
use usvg;

// self
use super::prelude::*;


pub fn draw(
    image: &usvg::Image,
    opt: &Options,
    cr: &cairo::Context,
) -> Rect {
    if image.format == usvg::ImageFormat::SVG {
        draw_svg(image, opt, cr);
    } else {
        draw_raster(image, cr);
    }

    image.view_box.rect
}

fn draw_raster(
    image: &usvg::Image,
    cr: &cairo::Context,
) {
    let r = image.view_box.rect;

    let img = match image.data {
        usvg::ImageData::Path(ref path) => {
            try_opt_warn!(gdk_pixbuf::Pixbuf::new_from_file(path).ok(), (),
                "Failed to load an external image: {:?}.", path)
        }
        usvg::ImageData::Raw(ref data) => {
            try_opt_warn!(load_raster_data(data), (),
                "Failed to load an embedded image.")
        }
    };

    let new_size = utils::apply_view_box(
        &image.view_box,
        ScreenSize::new(img.get_width() as u32, img.get_height() as u32),
    );

    let img = img.scale_simple(new_size.width as i32, new_size.height as i32,
                               gdk_pixbuf::InterpType::Bilinear);
    let img = try_opt_warn!(img, (), "Failed to scale an image.");

    let mut surface = try_create_surface!(new_size, ());

    {
        // Scaled image will be bigger than viewbox, so we have to
        // cut only the part specified by align rule.
        let (start_x, start_y, end_x, end_y) = if image.view_box.aspect.slice {
            let pos = utils::aligned_pos(
                image.view_box.aspect.align,
                0.0, 0.0, new_size.width as f64 - r.width, new_size.height as f64 - r.height,
            );

            (pos.x as u32, pos.y as u32, (pos.x + r.width) as u32, (pos.y + r.height) as u32)
        } else {
            (0, 0, img.get_width() as u32, img.get_height() as u32)
        };

        let mut surface_data = surface.get_data().unwrap();

        let channels = img.get_n_channels() as u32;
        let w = img.get_width() as u32;
        let h = img.get_height() as u32;
        let stride = img.get_rowstride() as u32;
        let img_pixels = unsafe { img.get_pixels() };
        // We can't iterate over pixels directly, because width may not be equal to stride.
        let mut i = 0;
        for y in 0..h {
            for x in 0..w {
                if x >= start_x && y >= start_y && x <= end_x && y <= end_y {
                    let idx = (y * stride + x * channels) as usize;

                    // NOTE: will not work on big endian.
                    if channels == 4 {
                        let r = img_pixels[idx + 0] as u32;
                        let g = img_pixels[idx + 1] as u32;
                        let b = img_pixels[idx + 2] as u32;
                        let a = img_pixels[idx + 3] as u32;

                        // https://www.cairographics.org/manual/cairo-Image-Surfaces.html#cairo-format-t
                        let tr = a * r + 0x80;
                        let tg = a * g + 0x80;
                        let tb = a * b + 0x80;
                        surface_data[i + 0] = (((tb >> 8) + tb) >> 8) as u8;
                        surface_data[i + 1] = (((tg >> 8) + tg) >> 8) as u8;
                        surface_data[i + 2] = (((tr >> 8) + tr) >> 8) as u8;
                        surface_data[i + 3] = a as u8;
                    } else {
                        surface_data[i + 0] = img_pixels[idx + 2];
                        surface_data[i + 1] = img_pixels[idx + 1];
                        surface_data[i + 2] = img_pixels[idx + 0];
                        surface_data[i + 3] = 255;
                    }
                }

                // Surface is always ARGB.
                i += 4;
            }
        }
    }

    let pos = utils::aligned_pos(
        image.view_box.aspect.align,
        r.x, r.y, r.width - img.get_width() as f64, r.height - img.get_height() as f64,
    );

    // We have to clip the image before rendering otherwise it will be
    // blurred outside the viewbox if `cr` has a transform.
    cr.rectangle(r.x, r.y, r.width, r.height);
    cr.clip();

    cr.set_source_surface(&surface, pos.x, pos.y);
    cr.paint();

    cr.reset_clip();
}

fn load_raster_data(data: &[u8]) -> Option<gdk_pixbuf::Pixbuf> {
    let loader = gdk_pixbuf::PixbufLoader::new();
    loader.write(data).ok()?;
    loader.close().ok()?;
    loader.get_pixbuf()
}

fn draw_svg(
    image: &usvg::Image,
    opt: &Options,
    cr: &cairo::Context,
) {
    let (tree, sub_opt) = try_opt!(utils::load_sub_svg(image, opt), ());

    let img_size = tree.svg_node().size.to_screen_size();
    let (ts, clip) = utils::prepare_sub_svg_geom(image, img_size);

    if let Some(clip) = clip {
        cr.rectangle(clip.x, clip.y, clip.width, clip.height);
        cr.clip();
    }

    cr.transform(ts.to_native());
    super::render_to_canvas(&tree, &sub_opt, img_size, cr);
    cr.reset_clip();
}
