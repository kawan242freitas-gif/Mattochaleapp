use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, TextureHandle, Vec2};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Icon {
    House,
    Search,
    ShoppingCart,
    Settings,
    UserRound,
    Wine,
    ArrowRight,
    Refresh,
    ChevronLeft,
    Plus,
    Minus,
    Copy,
    ImagePlus,
    Upload,
    Trash,
    Pencil,
    ShoppingBag,
    Star,
    CalendarDays,
    ChartColumns,
    BedDouble,
    BrushCleaning,
    ChevronRight,
    ChevronDown,
    Maximize,
    X,
}

pub fn load(ctx: &egui::Context) -> HashMap<Icon, TextureHandle> {
    [
        (Icon::House, include_str!("../assets/lucide/house.svg")),
        (Icon::Search, include_str!("../assets/lucide/search.svg")),
        (
            Icon::ShoppingCart,
            include_str!("../assets/lucide/shopping-cart.svg"),
        ),
        (
            Icon::Settings,
            include_str!("../assets/lucide/settings.svg"),
        ),
        (
            Icon::UserRound,
            include_str!("../assets/lucide/user-round.svg"),
        ),
        (Icon::Wine, include_str!("../assets/lucide/wine.svg")),
        (
            Icon::ArrowRight,
            include_str!("../assets/lucide/arrow-right.svg"),
        ),
        (
            Icon::Refresh,
            include_str!("../assets/lucide/refresh-cw.svg"),
        ),
        (
            Icon::ChevronLeft,
            include_str!("../assets/lucide/chevron-left.svg"),
        ),
        (Icon::Plus, include_str!("../assets/lucide/plus.svg")),
        (Icon::Minus, include_str!("../assets/lucide/minus.svg")),
        (Icon::Copy, include_str!("../assets/lucide/copy.svg")),
        (
            Icon::ImagePlus,
            include_str!("../assets/lucide/image-plus.svg"),
        ),
        (Icon::Upload, include_str!("../assets/lucide/upload.svg")),
        (Icon::Trash, include_str!("../assets/lucide/trash.svg")),
        (Icon::Pencil, include_str!("../assets/lucide/pencil.svg")),
        (
            Icon::ShoppingBag,
            include_str!("../assets/lucide/shopping-bag.svg"),
        ),
        (Icon::Star, include_str!("../assets/lucide/star.svg")),
        (
            Icon::CalendarDays,
            include_str!("../assets/lucide/calendar-days.svg"),
        ),
        (
            Icon::ChartColumns,
            include_str!("../assets/lucide/chart-no-axes-column-increasing.svg"),
        ),
        (
            Icon::BedDouble,
            include_str!("../assets/lucide/bed-double.svg"),
        ),
        (
            Icon::BrushCleaning,
            include_str!("../assets/lucide/brush-cleaning.svg"),
        ),
        (
            Icon::ChevronRight,
            include_str!("../assets/lucide/chevron-right.svg"),
        ),
        (
            Icon::ChevronDown,
            include_str!("../assets/lucide/chevron-down.svg"),
        ),
        (
            Icon::Maximize,
            include_str!("../assets/lucide/maximize.svg"),
        ),
        (Icon::X, include_str!("../assets/lucide/x.svg")),
    ]
    .into_iter()
    .filter_map(|(icon, svg)| rasterize(ctx, icon, svg).map(|texture| (icon, texture)))
    .collect()
}

fn rasterize(ctx: &egui::Context, icon: Icon, svg: &str) -> Option<TextureHandle> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(72, 72)?;
    // SVGs exported by lucide-react-native use a 96×96 intrinsic size with a
    // 24×24 viewBox. A fixed 3× scale clips most of the drawing into a corner;
    // fit the parsed document to the raster target instead.
    let size = tree.size();
    let scale = (pixmap.width() as f32 / size.width()).min(pixmap.height() as f32 / size.height());
    let offset_x = (pixmap.width() as f32 - size.width() * scale) / 2.0;
    let offset_y = (pixmap.height() as f32 - size.height() * scale) / 2.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_translate(offset_x, offset_y).pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    // Preserve the Lucide anti-aliased alpha mask and tint it in egui.
    let mut rgba = pixmap.data().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] > 0 {
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 255;
        }
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([72, 72], &rgba);
    Some(ctx.load_texture(
        format!("lucide-{icon:?}"),
        image,
        egui::TextureOptions::LINEAR,
    ))
}

pub fn image(
    ui: &mut egui::Ui,
    textures: &HashMap<Icon, TextureHandle>,
    icon: Icon,
    size: f32,
    tint: Color32,
) -> Response {
    if let Some(texture) = textures.get(&icon) {
        ui.add(
            egui::Image::new((texture.id(), Vec2::splat(size)))
                .fit_to_exact_size(Vec2::splat(size))
                .tint(tint),
        )
    } else {
        ui.allocate_response(Vec2::splat(size), Sense::hover())
    }
}

pub fn paint_in_rect(
    ui: &egui::Ui,
    textures: &HashMap<Icon, TextureHandle>,
    icon: Icon,
    rect: Rect,
    tint: Color32,
) {
    if let Some(texture) = textures.get(&icon) {
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            tint,
        );
    }
}
