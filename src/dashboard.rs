use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Layout, Margin, Pos2, Rect, RichText, Sense,
    Stroke, TextureHandle, Vec2,
};

use crate::{
    api,
    icons::{self, Icon},
    model::{CategoryFilter, Product},
    updater::{self, UpdateInfo},
};

const CANVAS: Color32 = Color32::from_rgb(25, 16, 11);
const SURFACE: Color32 = Color32::from_rgb(40, 25, 16);
const SURFACE_ALT: Color32 = Color32::from_rgb(52, 32, 19);
const INK: Color32 = Color32::from_rgb(246, 231, 207);
const MUTED: Color32 = Color32::from_rgb(190, 160, 126);
const GREEN: Color32 = Color32::from_rgb(153, 87, 38);
const GREEN_MID: Color32 = Color32::from_rgb(180, 104, 45);
const GREEN_PALE: Color32 = Color32::from_rgb(91, 54, 29);
const LINE: Color32 = Color32::from_rgb(94, 58, 32);
const ORANGE: Color32 = Color32::from_rgb(210, 133, 62);
const ORANGE_PALE: Color32 = Color32::from_rgb(77, 43, 22);
const RED: Color32 = Color32::from_rgb(174, 82, 52);
const RED_PALE: Color32 = Color32::from_rgb(73, 35, 23);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Section {
    #[default]
    Launcher,
    Reservas,
    Financas,
    Adega,
    Configuracoes,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsPage {
    #[default]
    Loja,
    Chales,
    Catalogo,
    Aparencia,
    Dados,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct AppPreferences {
    store_name: String,
    tagline: String,
    whatsapp: String,
    pix_key: String,
    chalet_names: Vec<String>,
    low_stock_threshold: u32,
    glass_effect: bool,
    compact_calendar: bool,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
struct UserProfile {
    name: String,
    #[serde(rename = "photoPath")]
    photo_path: Option<String>,
}

struct ProfileFrame {
    texture: TextureHandle,
    delay: Duration,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            store_name: "Matto Chalés".to_owned(),
            tagline: "Hospedagem e adega em um só lugar".to_owned(),
            whatsapp: "+55 35 9890-8891".to_owned(),
            pix_key: "matto.corporativo@gmail.com".to_owned(),
            chalet_names: (1..=5).map(|index| format!("Chalé {index:02}")).collect(),
            low_stock_threshold: 3,
            glass_effect: true,
            compact_calendar: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DayStatus {
    Disponivel,
    Bloqueada,
    Ocupada,
}

enum Event {
    Products(Result<Vec<Product>, String>),
    ProductImage(String, Result<image::RgbaImage, String>),
    AdminAuthorized(Result<(), String>),
    ProductSaved(Result<Product, String>),
    ProductDeleted(Result<String, String>),
    ProductImageUploaded(Result<String, String>),
    UpdateChecked(Result<Option<UpdateInfo>, String>),
    UpdateProgress(u8),
    UpdateDownloaded(Result<(PathBuf, String), String>),
}

#[derive(Default)]
struct ProductForm {
    name: String,
    label: String,
    price: String,
    promotional_price: String,
    stock_quantity: String,
    description: String,
    image_url: String,
    category: CategoryFilter,
    tone: String,
}

pub struct DashboardApp {
    section: Section,
    launcher_module: Section,
    launcher_rail_width: f32,
    month: u32,
    year: i32,
    selected_range: Option<(u32, u32)>,
    blocked_dates: HashSet<String>,
    occupied_dates: HashSet<String>,
    selected_chalet: u8,
    finance_month: u8,
    finance_filter: String,
    products: Vec<Product>,
    products_loading: bool,
    product_error: Option<String>,
    product_query: String,
    selected_product: Option<String>,
    product_images: HashMap<String, TextureHandle>,
    image_requests: HashSet<String>,
    hero_texture: Option<TextureHandle>,
    launcher_texture: Option<TextureHandle>,
    launcher_reservas_texture: Option<TextureHandle>,
    launcher_adega_texture: Option<TextureHandle>,
    launcher_reservas_button: Option<TextureHandle>,
    launcher_adega_button: Option<TextureHandle>,
    launcher_update_button: Option<TextureHandle>,
    brand_texture: Option<TextureHandle>,
    icons: HashMap<Icon, TextureHandle>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    notice: Option<String>,
    maximized: bool,
    availability_path: Option<PathBuf>,
    preferences: AppPreferences,
    preferences_path: Option<PathBuf>,
    profile_name: String,
    profile_frames: Vec<ProfileFrame>,
    profile_frame_index: usize,
    next_profile_frame: Instant,
    settings_page: SettingsPage,
    settings_notice: Option<String>,
    admin_token: String,
    admin_unlocked: bool,
    admin_busy: bool,
    admin_error: Option<String>,
    admin_form: ProductForm,
    editing_product_id: Option<String>,
    product_form_open: bool,
    image_upload_busy: bool,
    admin_category: CategoryFilter,
    last_context: egui::Context,
    update_available: Option<UpdateInfo>,
    update_busy: bool,
    update_progress: u8,
    update_message: Option<String>,
}

impl DashboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        set_theme(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let (blocked_dates, availability_path) = load_availability();
        let (preferences, preferences_path) = load_preferences();
        let (profile_name, profile_frames) = load_user_profile(&cc.egui_ctx);
        let next_profile_frame = Instant::now()
            + profile_frames
                .first()
                .map(|frame| frame.delay)
                .unwrap_or(Duration::from_secs(1));
        let hero_texture = load_texture(
            &cc.egui_ctx,
            "chalet-forest-header",
            include_bytes!("../assets/hero-forest.png"),
        );
        let launcher_texture = load_texture(
            &cc.egui_ctx,
            "matto-launcher-background",
            include_bytes!("../assets/launcher-wallpaper.jpg"),
        );
        let launcher_reservas_button = load_texture(
            &cc.egui_ctx,
            "launcher-open-reservations",
            include_bytes!("../assets/abrir-reservas.png"),
        );
        let launcher_adega_button = load_texture(
            &cc.egui_ctx,
            "launcher-open-cellar",
            include_bytes!("../assets/abrir-adega.png"),
        );
        let launcher_update_button = load_texture(
            &cc.egui_ctx,
            "launcher-update-pending",
            include_bytes!("../assets/atualizacao-pendente.png"),
        );
        let launcher_reservas_texture = load_texture(
            &cc.egui_ctx,
            "launcher-reservas-background",
            include_bytes!("../assets/reservas-wallpaper.png"),
        );
        let launcher_adega_texture = load_texture(
            &cc.egui_ctx,
            "launcher-adega-background",
            include_bytes!("../assets/adega-wallpaper.png"),
        );
        let brand_texture = load_texture(
            &cc.egui_ctx,
            "matto-chales-mark",
            include_bytes!("../assets/app-icon.png"),
        );
        let app = Self {
            section: Section::Launcher,
            launcher_module: Section::Reservas,
            launcher_rail_width: 72.0,
            month: 2,
            year: 2027,
            selected_range: Some((19, 21)),
            blocked_dates,
            occupied_dates: HashSet::new(),
            selected_chalet: 1,
            finance_month: 2,
            finance_filter: "Todas".to_owned(),
            products: Vec::new(),
            products_loading: true,
            product_error: None,
            product_query: String::new(),
            selected_product: None,
            product_images: HashMap::new(),
            image_requests: HashSet::new(),
            hero_texture,
            launcher_texture,
            launcher_reservas_texture,
            launcher_adega_texture,
            launcher_reservas_button,
            launcher_adega_button,
            launcher_update_button,
            brand_texture,
            icons: icons::load(&cc.egui_ctx),
            tx,
            rx,
            notice: None,
            maximized: true,
            availability_path,
            preferences,
            preferences_path,
            profile_name,
            profile_frames,
            profile_frame_index: 0,
            next_profile_frame,
            settings_page: SettingsPage::Loja,
            settings_notice: None,
            admin_token: String::new(),
            admin_unlocked: false,
            admin_busy: false,
            admin_error: None,
            admin_form: ProductForm::default(),
            editing_product_id: None,
            product_form_open: false,
            image_upload_busy: false,
            admin_category: CategoryFilter::Todos,
            last_context: cc.egui_ctx.clone(),
            update_available: None,
            update_busy: false,
            update_progress: 0,
            update_message: None,
        };
        app.request_products(cc.egui_ctx.clone());
        app.request_update_check(cc.egui_ctx.clone());
        app
    }

    fn request_update_check(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = updater::latest().map_err(|e| e.to_string());
            let _ = tx.send(Event::UpdateChecked(result));
            ctx.request_repaint();
        });
    }

    fn start_update_download(&mut self, ctx: egui::Context) {
        let Some(update) = self.update_available.clone() else { return; };
        self.update_busy = true;
        self.update_progress = 0;
        self.update_message = Some(format!("Baixando atualização {}…", update.version));
        let tx = self.tx.clone();
        thread::spawn(move || {
            let version = update.version.clone();
            let result = updater::download(&update, |percent| {
                let _ = tx.send(Event::UpdateProgress(percent));
                ctx.request_repaint();
            }).map(|path| (path, version)).map_err(|e| e.to_string());
            let _ = tx.send(Event::UpdateDownloaded(result));
            ctx.request_repaint();
        });
    }

    fn request_products(&self, ctx: egui::Context) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = api::fetch_products().map_err(|error| error.to_string());
            let _ = tx.send(Event::Products(result));
            ctx.request_repaint();
        });
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Products(Ok(products)) => {
                    self.products_loading = false;
                    self.product_error = None;
                    for product in &products {
                        self.request_product_image(product, ctx.clone());
                    }
                    self.products = products;
                }
                Event::UpdateChecked(Ok(update)) => { self.update_available = update; }
                Event::UpdateChecked(Err(error)) => { self.update_message = Some(format!("Não foi possível verificar atualizações: {error}")); }
                Event::UpdateProgress(percent) => { self.update_progress = percent; }
                Event::UpdateDownloaded(Ok((path, version))) => {
                    self.update_progress = 100;
                    self.update_message = Some("Aplicando atualização…".to_owned());
                    let launched = std::process::Command::new(path)
                        .arg("--apply-update")
                        .arg(std::process::id().to_string())
                        .arg(version)
                        .spawn();
                    match launched {
                        Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(error) => { self.update_busy = false; self.update_message = Some(format!("Falha ao iniciar a atualização: {error}")); }
                    }
                }
                Event::UpdateDownloaded(Err(error)) => { self.update_busy = false; self.update_message = Some(format!("Falha ao baixar atualização: {error}")); }
                Event::Products(Err(error)) => {
                    self.products_loading = false;
                    self.product_error = Some(error);
                }
                Event::ProductImage(id, Ok(image)) => {
                    let size = [image.width() as usize, image.height() as usize];
                    let texture = ctx.load_texture(
                        format!("adega-{id}"),
                        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                        egui::TextureOptions::LINEAR,
                    );
                    self.product_images.insert(id, texture);
                }
                Event::ProductImage(id, Err(_)) => {
                    self.image_requests.remove(&id);
                }
                Event::AdminAuthorized(Ok(())) => {
                    self.admin_busy = false;
                    self.admin_unlocked = true;
                    self.admin_error = None;
                    self.request_products(ctx.clone());
                    self.settings_notice = Some("Acesso ao catálogo autorizado.".to_owned());
                }
                Event::AdminAuthorized(Err(error)) => {
                    self.admin_busy = false;
                    self.admin_error = Some(format!("Não foi possível entrar: {error}"));
                }
                Event::ProductSaved(Ok(product)) => {
                    if let Some(index) = self.products.iter().position(|item| item.id == product.id)
                    {
                        self.products[index] = product.clone();
                    } else {
                        self.products.push(product.clone());
                    }
                    self.request_product_image(&product, ctx.clone());
                    self.product_form_open = false;
                    self.editing_product_id = None;
                    self.settings_notice = Some("Produto salvo no catálogo online.".to_owned());
                    self.admin_error = None;
                }
                Event::ProductSaved(Err(error)) => {
                    self.admin_error = Some(format!("Não foi possível salvar o produto: {error}"));
                }
                Event::ProductDeleted(Ok(id)) => {
                    self.products.retain(|product| product.id != id);
                    self.settings_notice = Some("Produto removido do catálogo.".to_owned());
                    self.admin_error = None;
                }
                Event::ProductDeleted(Err(error)) => {
                    self.admin_error = Some(format!("Não foi possível remover o produto: {error}"));
                }
                Event::ProductImageUploaded(Ok(url)) => {
                    self.image_upload_busy = false;
                    self.admin_form.image_url = url;
                    self.settings_notice = Some("Imagem enviada ao FiveManage.".to_owned());
                    self.admin_error = None;
                }
                Event::ProductImageUploaded(Err(error)) => {
                    self.image_upload_busy = false;
                    self.admin_error = Some(format!("Falha ao enviar imagem: {error}"));
                }
            }
        }
    }

    fn request_product_image(&mut self, product: &Product, ctx: egui::Context) {
        let Some(url) = product.image_url.as_deref() else {
            return;
        };
        if !self.image_requests.insert(product.id.clone()) {
            return;
        }
        let id = product.id.clone();
        let url = url.to_owned();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = api::fetch_image(&url).map_err(|error| error.to_string());
            let _ = tx.send(Event::ProductImage(id, result));
            ctx.request_repaint();
        });
    }

    fn title_bar(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        let launcher = self.section == Section::Launcher;
        let bar_fill = if launcher { Color32::from_rgb(31, 19, 12) } else { SURFACE };
        let title_color = if launcher { Color32::from_rgb(237, 238, 232) } else { INK };
        egui::Panel::top("matto-window-title")
            .exact_size(if launcher { 60.0 } else { 44.0 })
            .frame(
                Frame::new()
                    .fill(bar_fill)
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin::symmetric(15, 0)),
            )
            .show(root, |ui| {
                let rect = ui.max_rect();
                let drag_rect =
                    Rect::from_min_max(rect.min, Pos2::new(rect.right() - 145.0, rect.bottom()));
                let drag = ui.interact(drag_rect, ui.scope_id().with("window-drag"), Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.horizontal_centered(|ui| {
                    let brand_size = if launcher { 40.0 } else { 28.0 };
                    if let Some(brand) = &self.brand_texture {
                        ui.add(
                            egui::Image::new((brand.id(), Vec2::splat(brand_size)))
                                .fit_to_exact_size(Vec2::splat(brand_size)),
                        );
                    }
                    ui.allocate_ui_with_layout(
                        Vec2::new(220.0, brand_size),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.label(
                                    RichText::new(&self.preferences.store_name)
                                        .size(if launcher { 16.0 } else { 13.0 })
                                        .strong()
                                        .color(title_color),
                                );
                                ui.label(
                                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                        .size(if launcher { 10.0 } else { 9.0 })
                                        .color(MUTED),
                                );
                            });
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.window_control(ui, Icon::X, "Fechar") {
                            if self.section == Section::Launcher {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else {
                                if matches!(self.section, Section::Reservas | Section::Financas | Section::Adega) {
                                    self.launcher_module = self.section;
                                }
                                self.section = Section::Launcher;
                            }
                        }
                        if self.window_control(ui, Icon::Maximize, "Maximizar") {
                            self.maximized = !self.maximized;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                        }
                        if self.window_control(ui, Icon::Minus, "Minimizar") {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if !launcher && !self.profile_name.trim().is_empty() {
                            ui.add_space(8.0);
                            Frame::new()
                                .fill(Color32::from_rgb(38, 24, 16))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(76, 49, 31)))
                                .corner_radius(22.0)
                                .inner_margin(Margin::symmetric(10, 4))
                                .show(ui, |ui| {
                                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        if let Some(frame) = self.profile_frames.get(self.profile_frame_index) {
                                            ui.add(
                                                egui::Image::new((frame.texture.id(), Vec2::splat(26.0)))
                                                    .fit_to_exact_size(Vec2::splat(26.0))
                                                    .corner_radius(13.0),
                                            );
                                        } else {
                                            let (avatar_rect, _) = ui.allocate_exact_size(
                                                Vec2::splat(26.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().circle_filled(
                                                avatar_rect.center(),
                                                13.0,
                                                Color32::from_rgb(62, 46, 33),
                                            );
                                            icons::paint_in_rect(
                                                ui,
                                                &self.icons,
                                                Icon::UserRound,
                                                avatar_rect.shrink(6.0),
                                                INK,
                                            );
                                        }
                                        ui.label(
                                            RichText::new(&self.profile_name)
                                                .size(12.0)
                                                .strong()
                                                .color(INK),
                                        );
                                    });
                                });
                            ui.add_space(8.0);
                        }
                    });
                });
            });
    }

    fn window_control(&self, ui: &mut egui::Ui, icon: Icon, label: &str) -> bool {
        let icon_color = if self.section == Section::Launcher {
            Color32::from_rgb(225, 229, 222)
        } else {
            INK
        };
        let response = ui.add_sized(
            Vec2::new(42.0, 34.0),
            egui::Button::new("")
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .corner_radius(5),
        );
        if label == "Fechar" && response.hovered() {
            ui.painter().rect_filled(response.rect, 5.0, RED_PALE);
        }
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(response.rect.center(), Vec2::splat(14.0)),
            icon_color,
        );
        response.clicked()
    }

    fn show_launcher(&mut self, root: &mut egui::Ui) {
        const NIGHT: Color32 = Color32::from_rgb(19, 12, 8);
        const RAIL: Color32 = Color32::from_rgb(28, 18, 12);
        const TEXT: Color32 = Color32::from_rgb(246, 235, 215);
        const ACCENT: Color32 = Color32::from_rgb(196, 117, 52);

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(NIGHT)
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin::ZERO),
            )
            .show(root, |ui| {
                let bounds = ui.max_rect();
                let expanded_width = 242.0_f32.min(bounds.width() * 0.30);
                let current_rail = Rect::from_min_max(
                    bounds.min,
                    Pos2::new(bounds.left() + self.launcher_rail_width, bounds.bottom()),
                );
                let pointer_over_rail = ui
                    .ctx()
                    .pointer_hover_pos()
                    .is_some_and(|pointer| current_rail.contains(pointer));
                let target_width = if pointer_over_rail { expanded_width } else { 72.0 };
                let dt = ui.input(|input| input.stable_dt).clamp(0.0, 0.1);
                let easing = 1.0 - (-dt * 14.0).exp();
                self.launcher_rail_width += (target_width - self.launcher_rail_width) * easing;
                if (target_width - self.launcher_rail_width).abs() < 0.5 {
                    self.launcher_rail_width = target_width;
                } else {
                    ui.ctx().request_repaint_after(Duration::from_millis(16));
                }
                let rail_width = self.launcher_rail_width;
                let rail_rect = Rect::from_min_max(
                    bounds.min,
                    Pos2::new(bounds.left() + rail_width, bounds.bottom()),
                );
                let hero_rect = Rect::from_min_max(
                    Pos2::new(rail_rect.right(), bounds.top()),
                    bounds.max,
                );
                let painter = ui.painter().clone();

                painter.rect_filled(bounds, 0.0, NIGHT);
                let image_rect = hero_rect;
                let module_background = match self.launcher_module {
                    Section::Reservas => self.launcher_reservas_texture.as_ref(),
                    Section::Adega => self.launcher_adega_texture.as_ref(),
                    _ => self.launcher_texture.as_ref(),
                };
                if let Some(image) = module_background {
                    painter.image(
                        image.id(),
                        image_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                painter.rect_filled(
                    image_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(8, 13, 15, 30),
                );
                painter.rect_filled(rail_rect, 0.0, RAIL);
                painter.line_segment(
                    [Pos2::new(rail_rect.right(), rail_rect.top()), Pos2::new(rail_rect.right(), rail_rect.bottom())],
                    Stroke::new(1.0, Color32::from_rgb(43, 49, 51)),
                );

                let rail_collapsed = rail_width < 112.0;
                let profile_size = if rail_collapsed { 38.0 } else { 34.0 };
                // Mantém aproximadamente 2 cm de respiro entre o avatar e a cápsula dos módulos.
                let profile_capsule = Rect::from_center_size(
                    Pos2::new(rail_rect.center().x, rail_rect.center().y - 97.0),
                    Vec2::new(if rail_collapsed { 56.0 } else { rail_width - 20.0 }, 56.0),
                );
                let profile_center = Pos2::new(
                    if rail_collapsed { rail_rect.center().x } else { profile_capsule.left() + 28.0 },
                    profile_capsule.center().y,
                );
                painter.rect_filled(profile_capsule, 28.0, Color32::from_rgb(12, 16, 17));
                painter.rect_stroke(profile_capsule, 28.0, Stroke::new(1.0, Color32::from_rgb(43, 49, 51)), egui::StrokeKind::Inside);
                if !rail_collapsed {
                    painter.text(
                        Pos2::new(profile_capsule.left() + 56.0, profile_center.y),
                        Align2::LEFT_CENTER,
                        if self.profile_name.trim().is_empty() { "Perfil" } else { &self.profile_name },
                        FontId::proportional(16.0),
                        TEXT,
                    );
                }
                let profile_rect = Rect::from_center_size(profile_center, Vec2::splat(profile_size));
                let profile_response = if let Some(frame) = self.profile_frames.get(self.profile_frame_index) {
                    ui.put(
                        profile_rect,
                        egui::Image::new((frame.texture.id(), Vec2::splat(profile_size)))
                            .fit_to_exact_size(Vec2::splat(profile_size))
                            .corner_radius(profile_size * 0.5)
                            .sense(Sense::click()),
                    )
                } else {
                    painter.circle_filled(profile_center, profile_size * 0.5, Color32::from_rgb(62, 46, 33));
                    icons::paint_in_rect(ui, &self.icons, Icon::UserRound, profile_rect.shrink(8.0), TEXT);
                    ui.interact(profile_rect, ui.scope_id().with("launcher-profile"), Sense::click())
                };
                if profile_response.on_hover_text("Configurações").clicked() {
                    self.section = Section::Configuracoes;
                }

                if rail_collapsed {
                    let capsule = Rect::from_center_size(
                        Pos2::new(rail_rect.center().x, rail_rect.center().y + 65.0),
                        Vec2::new(56.0, 116.0),
                    );
                    painter.rect_filled(
                        capsule,
                        28.0,
                        Color32::from_rgb(12, 16, 17),
                    );
                    painter.rect_stroke(
                        capsule,
                        28.0,
                        Stroke::new(1.0, Color32::from_rgb(43, 49, 51)),
                        egui::StrokeKind::Inside,
                    );
                }
                let mut open_module = None;
                let nav_rect = Rect::from_center_size(
                    Pos2::new(rail_rect.center().x, rail_rect.center().y + 65.0),
                    Vec2::new(rail_width, 96.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(nav_rect), |rail| {
                    rail.with_layout(Layout::top_down(Align::Min), |rail| {
                        rail.spacing_mut().item_spacing.y = if rail_collapsed { 4.0 } else { 0.0 };
                        if self.launcher_rail_item(
                            rail,
                            Icon::CalendarDays,
                            "Reservas",
                            self.launcher_module == Section::Reservas,
                        ) {
                            self.launcher_module = Section::Reservas;
                        }
                        if self.launcher_rail_item(
                            rail,
                            Icon::Wine,
                            "Adega",
                            self.launcher_module == Section::Adega,
                        ) {
                            self.launcher_module = Section::Adega;
                        }
                    });
                });

                let (title, subtitle, button_text) = match self.launcher_module {
                    Section::Reservas => (
                        "RESERVAS",
                        "Configure as datas disponíveis dos chalés.",
                        "ABRIR RESERVAS",
                    ),
                    Section::Financas => (
                        "FINANÇAS",
                        "Acompanhe receitas, despesas e movimentações.",
                        "ABRIR FINANÇAS",
                    ),
                    Section::Adega => (
                        "ADEGA",
                        "Gerencie produtos, estoque e pedidos.",
                        "ABRIR ADEGA",
                    ),
                    _ => (
                        "MATTO CHALÉS",
                        "Gerencie a operação dos seus chalés.",
                        "ABRIR MÓDULO",
                    ),
                };
                let title_size = (hero_rect.width() * 0.064).clamp(58.0, 98.0);
                let copy_left = image_rect.left() + (image_rect.width() * 0.045).max(42.0);
                let copy_top = image_rect.top() + image_rect.height() * 0.30;
                let title_pos = Pos2::new(copy_left, copy_top);
                painter.text(
                    title_pos,
                    Align2::LEFT_TOP,
                    title,
                    FontId::proportional(title_size),
                    TEXT,
                );
                let subtitle_pos = Pos2::new(copy_left + 3.0, copy_top + title_size + 20.0);
                painter.text(
                    subtitle_pos,
                    Align2::LEFT_TOP,
                    subtitle,
                    FontId::proportional(25.0),
                    Color32::from_rgb(244, 242, 234),
                );
                let button_pos = Pos2::new(copy_left, subtitle_pos.y + 62.0);
                let is_reservas = self.launcher_module == Section::Reservas;
                let update_mode = self.update_available.is_some() || self.update_busy;
                let has_image_button = update_mode
                    || (is_reservas || self.launcher_module == Section::Adega);
                let button_label = if self.update_busy { "ATUALIZAÇÃO PENDENTE".to_owned() }
                    else if let Some(update) = &self.update_available { format!("ATUALIZAR PARA {}", update.version) }
                    else { button_text.to_owned() };
                let button_size = if has_image_button {
                    Vec2::new(480.0, 110.0)
                } else {
                    Vec2::new(if update_mode { 480.0 } else { 374.0 }, 82.0)
                };
                let button_rect = Rect::from_min_size(button_pos, button_size);
                let button = if has_image_button {
                    let image_button = if update_mode {
                        self.launcher_update_button.as_ref()
                    } else if is_reservas {
                        self.launcher_reservas_button.as_ref()
                    } else {
                        self.launcher_adega_button.as_ref()
                    };
                    if let Some(texture) = image_button {
                        ui.put(
                            button_rect,
                            egui::Image::new((texture.id(), button_size))
                                .fit_to_exact_size(button_size)
                                .sense(if self.update_busy {
                                    Sense::hover()
                                } else {
                                    Sense::click()
                                }),
                        )
                    } else {
                        ui.put(
                            button_rect,
                            egui::Button::new(RichText::new(&button_label).size(24.0).strong())
                                .fill(ACCENT)
                                .corner_radius(9),
                        )
                    }
                } else {
                    let response = ui.put(
                        button_rect,
                        egui::Button::new(
                            RichText::new(&button_label)
                                .size(24.0)
                                .strong()
                                .color(Color32::from_rgb(30, 24, 20)),
                        )
                        .fill(ACCENT)
                        .corner_radius(9),
                    );
                    icons::paint_in_rect(
                        ui,
                        &self.icons,
                        Icon::ArrowRight,
                        Rect::from_center_size(
                            Pos2::new(button_rect.right() - 42.0, button_rect.center().y),
                            Vec2::splat(25.0),
                        ),
                        Color32::from_rgb(30, 24, 20),
                    );
                    response
                };
                if update_mode {
                    if !self.update_busy {
                        if let Some(message) = &self.update_message {
                            painter.text(
                                Pos2::new(button_rect.left(), button_rect.bottom() + 18.0),
                                Align2::LEFT_TOP,
                                message,
                                FontId::proportional(15.0),
                                TEXT,
                            );
                        }
                    }
                    if self.update_busy {
                        let bar = Rect::from_min_size(Pos2::new(button_rect.left(), button_rect.bottom() + 20.0), Vec2::new(button_rect.width(), 12.0));
                        painter.rect_filled(bar, 6.0, Color32::from_rgb(49, 31, 20));
                        painter.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * self.update_progress as f32 / 100.0, bar.height())), 6.0, ACCENT);
                    }
                }
                if button.clicked() {
                    if update_mode {
                        if !self.update_busy {
                            self.start_update_download(ui.ctx().clone());
                        }
                    } else {
                        open_module = Some(self.launcher_module);
                    }
                }
                if let Some(section) = open_module {
                    self.section = section;
                }
            });
    }

    fn launcher_rail_item(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        label: &str,
        selected: bool,
    ) -> bool {
        let width = ui.available_width();
        let expanded = width > 112.0;
        let item_height = if expanded { 68.0 } else { 46.0 };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, item_height), Sense::click());
        let visual_rect = if expanded {
            rect
        } else {
            Rect::from_center_size(rect.center(), Vec2::new(46.0, 46.0))
        };
        if selected || response.hovered() {
            ui.painter().rect_filled(
                visual_rect,
                if expanded { 0.0 } else { 23.0 },
                if selected {
                    if expanded {
                        Color32::from_rgb(48, 55, 58)
                    } else {
                        Color32::from_rgb(72, 44, 25)
                    }
                } else {
                    Color32::from_rgb(33, 39, 41)
                },
            );
        }
        if selected && expanded {
            ui.painter().rect_filled(
                Rect::from_min_max(rect.min, Pos2::new(rect.left() + 6.0, rect.bottom())),
                0.0,
                Color32::from_rgb(210, 139, 83),
            );
        }
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(
                Pos2::new(
                    rect.left() + if expanded { 54.0 } else { rect.width() * 0.5 },
                    rect.center().y,
                ),
                Vec2::splat(if expanded { 31.0 } else { 23.0 }),
            ),
                if selected && !expanded {
                    Color32::from_rgb(224, 155, 88)
                } else {
                    Color32::from_rgb(232, 234, 230)
                },
        );
        if expanded {
            ui.painter().text(
                Pos2::new(rect.left() + 99.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(22.0),
                Color32::from_rgb(241, 241, 237),
            );
        }
        response.on_hover_text(label).clicked()
    }
    fn navigation(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("matto-floating-navigation")
            .exact_size(94.0)
            .frame(
                Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(239, 237, 229, 212))
                    .inner_margin(Margin::symmetric(10, 6)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    let dock_width = 760.0;
                    let left_gap = ((ui.available_width() - dock_width) * 0.5).max(0.0);
                    ui.add_space(left_gap);
                    ui.allocate_ui_with_layout(
                        Vec2::new(dock_width, 64.0),
                        Layout::top_down(Align::Center),
                        |ui| {
                            Frame::new()
                                .fill(if self.preferences.glass_effect {
                                    Color32::from_rgba_unmultiplied(250, 249, 244, 222)
                                } else {
                                    SURFACE
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    Color32::from_rgba_unmultiplied(255, 255, 255, 235),
                                ))
                                .corner_radius(30)
                                .inner_margin(Margin::symmetric(9, 3))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        self.nav_tab(
                                            ui,
                                            Section::Reservas,
                                            Icon::CalendarDays,
                                            "Reservas",
                                        );
                                        self.nav_tab(
                                            ui,
                                            Section::Financas,
                                            Icon::ChartColumns,
                                            "Finanças",
                                        );
                                        self.nav_tab(ui, Section::Adega, Icon::Wine, "Adega");
                                        self.nav_tab(
                                            ui,
                                            Section::Configuracoes,
                                            Icon::Settings,
                                            "Configurações",
                                        );
                                    });
                                });
                        },
                    );
                });
            });
    }

    fn nav_tab(&mut self, ui: &mut egui::Ui, section: Section, icon: Icon, label: &str) {
        let selected = self.section == section;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(174.0, 56.0), Sense::click());
        if selected || response.hovered() {
            let fill = if selected {
                Color32::from_rgba_unmultiplied(217, 229, 208, 215)
            } else {
                Color32::from_rgba_unmultiplied(232, 231, 222, 140)
            };
            ui.painter().rect_filled(rect, 28.0, fill);
        }
        let tint = if selected { INK } else { MUTED };
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 18.0),
                Vec2::splat(21.0),
            ),
            tint,
        );
        ui.painter().text(
            Pos2::new(rect.center().x, rect.top() + 42.0),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(if label == "Configurações" {
                12.5
            } else {
                14.0
            }),
            tint,
        );
        if response.clicked() {
            self.section = section;
        }
    }

    fn hero(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 142.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 16.0, Color32::from_rgb(30, 49, 36));
        if let Some(texture) = &self.hero_texture {
            let image_rect = Rect::from_min_max(
                Pos2::new(rect.right() - rect.width() * 0.58, rect.top()),
                rect.right_bottom(),
            );
            ui.painter().image(
                texture.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            let fade_width = 260.0;
            for index in 0..20 {
                let x = image_rect.left() + index as f32 * (fade_width / 20.0);
                let alpha = ((20 - index) as f32 / 20.0 * 245.0) as u8;
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x, rect.top()),
                        Pos2::new(x + fade_width / 20.0 + 1.0, rect.bottom()),
                    ),
                    0.0,
                    Color32::from_rgba_unmultiplied(30, 49, 36, alpha),
                );
            }
        }
        ui.painter().text(
            Pos2::new(rect.left() + 58.0, rect.center().y),
            Align2::LEFT_CENTER,
            "Disponibilidade",
            FontId::proportional(40.0),
            Color32::WHITE,
        );
        ui.painter().text(
            Pos2::new(rect.left() + 58.6, rect.center().y),
            Align2::LEFT_CENTER,
            "Disponibilidade",
            FontId::proportional(40.0),
            Color32::WHITE,
        );

        let selector = Rect::from_min_size(
            Pos2::new(rect.right() - 282.0, rect.center().y - 23.0),
            Vec2::new(240.0, 46.0),
        );
        let response = ui.interact(
            selector,
            ui.scope_id().with("chalet-picker"),
            Sense::click(),
        );
        ui.painter().rect_filled(
            selector,
            23.0,
            Color32::from_rgba_unmultiplied(247, 248, 235, 226),
        );
        ui.painter().rect_stroke(
            selector,
            23.0,
            Stroke::new(1.0, Color32::from_white_alpha(150)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            Pos2::new(selector.left() + 20.0, selector.center().y),
            Align2::LEFT_CENTER,
            self.preferences
                .chalet_names
                .get(self.selected_chalet.saturating_sub(1) as usize)
                .cloned()
                .unwrap_or_else(|| format!("Chalé {:02}", self.selected_chalet)),
            FontId::proportional(17.0),
            INK,
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            Icon::ChevronDown,
            Rect::from_center_size(
                Pos2::new(selector.right() - 22.0, selector.center().y),
                Vec2::splat(18.0),
            ),
            INK,
        );
        if response.clicked() {
            let chalet_count = self.preferences.chalet_names.len().clamp(1, 99) as u8;
            self.selected_chalet = self.selected_chalet % chalet_count + 1;
        }
    }

    fn show_reservas(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);
                ui.add_space(0.0);
                self.hero(ui);
                ui.add_space(34.0);

                let total_width = ui.available_width();
                let panel_height = 347.0;
                ui.horizontal(|ui| {
                    let calendar_width = total_width * 0.645;
                    ui.allocate_ui_with_layout(
                        Vec2::new(calendar_width, panel_height),
                        Layout::top_down(Align::Min),
                        |ui| self.calendar_panel(ui),
                    );
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(16.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(
                            (total_width - calendar_width - 80.0).max(300.0),
                            panel_height,
                        ),
                        Layout::top_down(Align::Min),
                        |ui| self.editor_panel(ui),
                    );
                });
            });
    }

    fn calendar_panel(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, Color32::WHITE))
            .corner_radius(18)
            .inner_margin(Margin::symmetric(24, 18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.icon_button(
                        ui,
                        Icon::ChevronLeft,
                        42.0,
                        SURFACE_ALT,
                        INK,
                        "Mes anterior",
                    );
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!("{} {}", month_name(self.month), self.year))
                                .size(25.0)
                                .strong()
                                .color(INK),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.icon_button(
                            ui,
                            Icon::ArrowRight,
                            42.0,
                            SURFACE_ALT,
                            INK,
                            "Proximo mes",
                        );
                    });
                });
                ui.add_space(10.0);

                let weekdays = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
                let cell_width = (ui.available_width() / 7.0).min(94.0);
                egui::Grid::new("availability-calendar")
                    .num_columns(7)
                    .spacing(Vec2::new(4.0, 4.0))
                    .show(ui, |ui| {
                        for weekday in weekdays {
                            ui.add_sized(
                                Vec2::new(cell_width, 26.0),
                                egui::Label::new(
                                    RichText::new(weekday).size(14.0).strong().color(MUTED),
                                )
                                .halign(egui::Align::Center),
                            );
                        }
                        ui.end_row();
                        let first_weekday = weekday_monday(self.year, self.month);
                        let days = days_in_month(self.year, self.month);
                        for _ in 0..first_weekday {
                            ui.add_sized(
                                Vec2::new(
                                    cell_width,
                                    if self.preferences.compact_calendar {
                                        32.0
                                    } else {
                                        38.0
                                    },
                                ),
                                egui::Label::new(""),
                            );
                        }
                        for day in 1..=days {
                            let status = self.day_status(day);
                            let selected = self.day_selected(day);
                            let fill = if selected {
                                GREEN_MID
                            } else {
                                match status {
                                    DayStatus::Disponivel => Color32::TRANSPARENT,
                                    DayStatus::Bloqueada => ORANGE_PALE,
                                    DayStatus::Ocupada => RED_PALE,
                                }
                            };
                            let color = if selected { Color32::WHITE } else { INK };
                            let response = ui.add_sized(
                                Vec2::new(
                                    cell_width,
                                    if self.preferences.compact_calendar {
                                        32.0
                                    } else {
                                        38.0
                                    },
                                ),
                                egui::Button::new(
                                    RichText::new(day.to_string()).size(17.0).color(color),
                                )
                                .fill(fill)
                                .stroke(Stroke::NONE)
                                .corner_radius(18),
                            );
                            if response.clicked() {
                                self.select_day(day);
                            }
                            if (first_weekday + day as usize) % 7 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    fn editor_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(1.0);
        ui.label(RichText::new("Editar datas").size(25.0).strong().color(INK));
        ui.add_space(8.0);
        let date_rect = ui
            .allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover())
            .0;
        ui.painter().rect_filled(date_rect, 22.0, SURFACE);
        ui.painter().rect_stroke(
            date_rect,
            22.0,
            Stroke::new(1.0, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            Icon::CalendarDays,
            Rect::from_center_size(
                Pos2::new(date_rect.left() + 26.0, date_rect.center().y),
                Vec2::splat(20.0),
            ),
            INK,
        );
        ui.painter().text(
            Pos2::new(date_rect.left() + 50.0, date_rect.center().y),
            Align2::LEFT_CENTER,
            self.range_label(),
            FontId::proportional(16.0),
            INK,
        );
        ui.add_space(12.0);
        let horizontal_gaps = ui.spacing().item_spacing.x * 2.0 + 8.0;
        let button_width = ((ui.available_width() - horizontal_gaps) * 0.5).max(120.0);
        ui.horizontal(|ui| {
            self.action_button(
                ui,
                "Bloquear",
                ORANGE_PALE,
                ORANGE,
                button_width,
                Self::block_selected,
            );
            ui.add_space(8.0);
            self.action_button(
                ui,
                "Liberar",
                GREEN_PALE,
                INK,
                button_width,
                Self::release_selected,
            );
        });
        ui.add_space(10.0);
        if self.primary_button(ui, "Salvar datas", GREEN) {
            self.save_availability();
        }
        if let Some(notice) = &self.notice {
            ui.add_space(6.0);
            ui.label(RichText::new(notice).size(12.0).color(GREEN));
        }
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            self.legend(ui, GREEN_MID, "Disponível");
            self.legend(ui, Color32::from_rgb(226, 179, 136), "Bloqueada");
            self.legend(ui, RED, "Ocupada");
        });
    }

    fn show_financas(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Finanças").size(34.0).strong().color(INK));
                    ui.add_space(16.0);
                    egui::ComboBox::from_id_salt("finance-month")
                        .selected_text(month_name(self.finance_month as u32))
                        .width(148.0)
                        .show_ui(ui, |ui| {
                            for month in 1..=12 {
                                ui.selectable_value(
                                    &mut self.finance_month,
                                    month,
                                    month_name(month as u32),
                                );
                            }
                        });
                });
                ui.add_space(18.0);
                let metric_gaps = ui.spacing().item_spacing.x * 2.0 + 14.0 * 2.0;
                let metric_width = ((ui.available_width() - metric_gaps) / 3.0).max(210.0);
                ui.horizontal(|ui| {
                    self.metric_card(ui, "Saldo do mês", "R$ 8.420,00", "", INK, metric_width);
                    ui.add_space(14.0);
                    self.metric_card(
                        ui,
                        "Receitas",
                        "R$ 12.380,00",
                        "Hospedagem e vendas",
                        GREEN,
                        metric_width,
                    );
                    ui.add_space(14.0);
                    self.metric_card(
                        ui,
                        "Despesas",
                        "R$ 3.960,00",
                        "Custos operacionais",
                        ORANGE,
                        metric_width,
                    );
                });
                ui.add_space(26.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Movimentações")
                            .size(23.0)
                            .strong()
                            .color(INK),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("finance-filter")
                            .selected_text(&self.finance_filter)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.finance_filter,
                                    "Todas".to_owned(),
                                    "Todas",
                                );
                                ui.selectable_value(
                                    &mut self.finance_filter,
                                    "Receitas".to_owned(),
                                    "Receitas",
                                );
                                ui.selectable_value(
                                    &mut self.finance_filter,
                                    "Despesas".to_owned(),
                                    "Despesas",
                                );
                            });
                    });
                });
                ui.add_space(12.0);
                Frame::new()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0, Color32::WHITE))
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        if self.finance_filter != "Despesas" {
                            self.finance_row(
                                ui,
                                Icon::BedDouble,
                                GREEN_PALE,
                                "Hospedagem",
                                "Chalé 01 · 19 de fevereiro",
                                "R$ 7.920,00",
                                true,
                            );
                            ui.separator();
                        }
                        if self.finance_filter != "Receitas" {
                            self.finance_row(
                                ui,
                                Icon::BrushCleaning,
                                ORANGE_PALE,
                                "Limpeza",
                                "Serviço · 21 de fevereiro",
                                "R$ 240,00",
                                false,
                            );
                            ui.separator();
                        }
                        if self.finance_filter != "Despesas" {
                            self.finance_row(
                                ui,
                                Icon::Wine,
                                GREEN_PALE,
                                "Adega",
                                "Venda de produtos · 22 de fevereiro",
                                "R$ 460,00",
                                true,
                            );
                        }
                    });
            });
    }

    fn show_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Configurações")
                                .size(32.0)
                                .strong()
                                .color(INK),
                        );
                        ui.label(
                            RichText::new("Personalize a operação da Matto Chalés")
                                .size(13.0)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let save = ui.add_sized(
                            Vec2::new(178.0, 42.0),
                            egui::Button::new(
                                RichText::new("Salvar alterações")
                                    .size(14.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(GREEN)
                            .stroke(Stroke::NONE)
                            .corner_radius(21),
                        );
                        if save.clicked() {
                            self.save_preferences();
                        }
                    });
                });
                ui.add_space(18.0);

                let settings_height = ui.available_height().max(500.0);
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(250.0, settings_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            Frame::new()
                                .fill(Color32::from_rgba_unmultiplied(250, 249, 244, 224))
                                .stroke(Stroke::new(1.0, Color32::WHITE))
                                .corner_radius(18)
                                .inner_margin(Margin::symmetric(12, 12))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("GERENCIAMENTO")
                                            .size(10.0)
                                            .strong()
                                            .color(MUTED),
                                    );
                                    ui.add_space(8.0);
                                    for (page, icon, label, description) in [
                                        (
                                            SettingsPage::Loja,
                                            Icon::House,
                                            "Loja",
                                            "Perfil e contato",
                                        ),
                                        (
                                            SettingsPage::Chales,
                                            Icon::BedDouble,
                                            "Chalés",
                                            "Unidades e reservas",
                                        ),
                                        (
                                            SettingsPage::Catalogo,
                                            Icon::Wine,
                                            "Catálogo",
                                            "Produtos e estoque",
                                        ),
                                        (
                                            SettingsPage::Aparencia,
                                            Icon::Settings,
                                            "Aparência",
                                            "Visual do aplicativo",
                                        ),
                                        (
                                            SettingsPage::Dados,
                                            Icon::Refresh,
                                            "Dados e conexão",
                                            "Sincronização e arquivos",
                                        ),
                                    ] {
                                        self.settings_nav_item(ui, page, icon, label, description);
                                    }
                                });
                        },
                    );
                    ui.add_space(16.0);
                    let content_width = (ui.available_width() - 28.0).max(400.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(content_width, settings_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            Frame::new()
                                .fill(Color32::from_rgba_unmultiplied(250, 249, 244, 232))
                                .stroke(Stroke::new(1.0, Color32::WHITE))
                                .corner_radius(18)
                                .inner_margin(Margin::symmetric(22, 18))
                                .show(ui, |ui| {
                                    match self.settings_page {
                                        SettingsPage::Loja => self.settings_store(ui),
                                        SettingsPage::Chales => self.settings_chalets(ui),
                                        SettingsPage::Catalogo => self.settings_catalog(ui, ctx),
                                        SettingsPage::Aparencia => self.settings_appearance(ui),
                                        SettingsPage::Dados => self.settings_data(ui),
                                    }
                                    if let Some(notice) = &self.settings_notice {
                                        ui.add_space(14.0);
                                        ui.label(RichText::new(notice).size(12.0).color(GREEN));
                                    }
                                });
                        },
                    );
                });
            });
    }

    fn settings_nav_item(
        &mut self,
        ui: &mut egui::Ui,
        page: SettingsPage,
        icon: Icon,
        title: &str,
        description: &str,
    ) {
        let selected = self.settings_page == page;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                12.0,
                if selected {
                    Color32::from_rgba_unmultiplied(217, 229, 208, 215)
                } else {
                    Color32::from_rgba_unmultiplied(232, 231, 222, 155)
                },
            );
        }
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(
                Pos2::new(rect.left() + 23.0, rect.center().y),
                Vec2::splat(19.0),
            ),
            if selected { GREEN } else { MUTED },
        );
        ui.painter().text(
            Pos2::new(rect.left() + 44.0, rect.center().y - 7.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(13.0),
            INK,
        );
        ui.painter().text(
            Pos2::new(rect.left() + 44.0, rect.center().y + 10.0),
            Align2::LEFT_CENTER,
            description,
            FontId::proportional(10.0),
            MUTED,
        );
        if response.clicked() {
            self.settings_page = page;
            self.settings_notice = None;
        }
    }

    fn settings_store(&mut self, ui: &mut egui::Ui) {
        settings_heading(
            ui,
            "Perfil da loja",
            "Identidade e formas de contato exibidas aos clientes.",
        );
        ui.add_space(14.0);
        settings_field(
            ui,
            "Nome da loja",
            &mut self.preferences.store_name,
            "Matto Chalés",
        );
        settings_field(
            ui,
            "Apresentação",
            &mut self.preferences.tagline,
            "Frase curta para sua loja",
        );
        ui.columns(2, |columns| {
            settings_field(
                &mut columns[0],
                "WhatsApp para pedidos",
                &mut self.preferences.whatsapp,
                "+55 00 00000-0000",
            );
            settings_field(
                &mut columns[1],
                "Chave Pix",
                &mut self.preferences.pix_key,
                "E-mail, telefone ou chave aleatória",
            );
        });
        ui.add_space(10.0);
        Frame::new()
            .fill(SURFACE_ALT)
            .corner_radius(12)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::image(ui, &self.icons, Icon::ShoppingBag, 19.0, GREEN);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Dados usados no checkout").strong().color(INK));
                        ui.label(RichText::new("Preferências guardadas localmente no aplicativo; ainda não alteram o checkout do site.").size(11.0).color(MUTED));
                    });
                });
            });
    }

    fn settings_chalets(&mut self, ui: &mut egui::Ui) {
        settings_heading(
            ui,
            "Unidades e reservas",
            "Organize os nomes das unidades e preferências do calendário.",
        );
        ui.add_space(14.0);
        ui.label(
            RichText::new("Unidades cadastradas")
                .size(13.0)
                .strong()
                .color(INK),
        );
        ui.label(
            RichText::new("O seletor da tela de disponibilidade acompanha a lista abaixo.")
                .size(11.0)
                .color(MUTED),
        );
        ui.add_space(8.0);
        for (index, name) in self.preferences.chalet_names.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:02}", index + 1))
                        .size(12.0)
                        .strong()
                        .color(GREEN),
                );
                ui.add_sized(
                    Vec2::new(420.0, 34.0),
                    egui::TextEdit::singleline(name).hint_text("Nome da unidade"),
                );
            });
        }
        ui.add_space(15.0);
        ui.checkbox(
            &mut self.preferences.compact_calendar,
            "Usar calendário compacto",
        );
        ui.label(
            RichText::new(
                "A ocupação e os bloqueios ainda são armazenados localmente neste computador.",
            )
            .size(11.0)
            .color(MUTED),
        );
    }

    fn settings_catalog(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        settings_heading(
            ui,
            "Catálogo e estoque",
            "Cadastre, edite e organize os produtos da loja.",
        );
        ui.add_space(14.0);
        if !self.admin_unlocked {
            ui.label(
                RichText::new(
                    "Entre com sua chave administrativa para gerenciar o catálogo online.",
                )
                .size(12.0)
                .color(MUTED),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_sized(
                    Vec2::new(380.0, 38.0),
                    egui::TextEdit::singleline(&mut self.admin_token)
                        .password(true)
                        .hint_text("Chave de administração"),
                );
                let unlock = ui.add_enabled(
                    !self.admin_busy,
                    egui::Button::new(
                        RichText::new(if self.admin_busy {
                            "Verificando…"
                        } else {
                            "Desbloquear catálogo"
                        })
                        .color(Color32::WHITE)
                        .strong(),
                    )
                    .fill(GREEN)
                    .corner_radius(18),
                );
                if unlock.clicked() {
                    self.authenticate_admin(ctx.clone());
                }
            });
            if let Some(error) = &self.admin_error {
                ui.add_space(8.0);
                ui.label(RichText::new(error).size(12.0).color(RED));
            }
            ui.add_space(12.0);
            ui.label(RichText::new("A chave é enviada ao servidor para validação e não é gravada nas preferências locais.").size(11.0).color(MUTED));
            return;
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("{} produtos", self.products.len()))
                        .size(13.0)
                        .strong()
                        .color(INK),
                );
                ui.label(
                    RichText::new("Gerencie itens por categoria, estoque e preço promocional.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        Vec2::new(150.0, 38.0),
                        egui::Button::new(
                            RichText::new("+ Novo produto")
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .fill(GREEN)
                        .corner_radius(19),
                    )
                    .clicked()
                {
                    self.reset_product_form();
                }
                ui.add_space(8.0);
                if self.text_button(ui, Icon::Refresh, "Sincronizar", 130.0, 38.0) {
                    self.products_loading = true;
                    self.product_error = None;
                    self.request_products(ctx.clone());
                }
                ui.add_space(8.0);
                if ui.button("Bloquear").clicked() {
                    self.admin_unlocked = false;
                    self.admin_token.clear();
                    self.product_form_open = false;
                    self.editing_product_id = None;
                }
            });
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            for (filter, label) in [
                (CategoryFilter::Todos, "Todos"),
                (CategoryFilter::Adega, "Adega"),
                (CategoryFilter::Frigobar, "Frigobar"),
            ] {
                let selected = self.admin_category == filter;
                if ui
                    .add(
                        egui::Button::new(label)
                            .fill(if selected { GREEN_PALE } else { SURFACE_ALT })
                            .stroke(Stroke::new(1.0, LINE))
                            .corner_radius(16),
                    )
                    .clicked()
                {
                    self.admin_category = filter;
                }
            }
        });
        ui.add_space(10.0);

        if self.product_form_open {
            self.settings_product_form(ui, ctx);
            ui.add_space(12.0);
        }

        if let Some(error) = &self.admin_error {
            ui.label(RichText::new(error).size(12.0).color(RED));
        }

        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                let category = self.admin_category;
                let products: Vec<Product> = self
                    .products
                    .iter()
                    .filter(|product| {
                        category == CategoryFilter::Todos
                            || (category == CategoryFilter::Adega && product.category == "adega")
                            || (category == CategoryFilter::Frigobar
                                && product.category == "frigobar")
                    })
                    .cloned()
                    .collect();
                for product in products {
                    self.admin_product_row(ui, &product);
                }
                if self.products.is_empty() && !self.products_loading {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(
                            "Nenhum produto cadastrado. Crie o primeiro item do catálogo.",
                        )
                        .color(MUTED),
                    );
                }
            });

        ui.add_space(12.0);
        ui.label(
            RichText::new("Alerta de estoque baixo")
                .size(12.0)
                .strong()
                .color(INK),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.preferences.low_stock_threshold)
                    .range(0..=999)
                    .suffix(" unidades"),
            );
            ui.label(
                RichText::new("Este limite é uma preferência local do aplicativo.")
                    .size(11.0)
                    .color(MUTED),
            );
        });
    }

    fn settings_product_form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        Frame::new()
            .fill(SURFACE_ALT)
            .corner_radius(14)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(if self.editing_product_id.is_some() {
                        "Editar produto"
                    } else {
                        "Novo produto"
                    })
                    .size(16.0)
                    .strong()
                    .color(INK),
                );
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    settings_field(
                        &mut columns[0],
                        "Nome do produto",
                        &mut self.admin_form.name,
                        "Ex.: Vinho tinto",
                    );
                    settings_field(
                        &mut columns[1],
                        "Título do cartão",
                        &mut self.admin_form.label,
                        "Nome exibido na loja",
                    );
                    settings_field(
                        &mut columns[0],
                        "Preço normal",
                        &mut self.admin_form.price,
                        "R$ 0,00",
                    );
                    settings_field(
                        &mut columns[1],
                        "Preço promocional",
                        &mut self.admin_form.promotional_price,
                        "Opcional",
                    );
                    columns[0].label(RichText::new("Categoria").size(12.0).strong().color(INK));
                    columns[0].horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.admin_form.category,
                            CategoryFilter::Adega,
                            "Adega",
                        );
                        ui.selectable_value(
                            &mut self.admin_form.category,
                            CategoryFilter::Frigobar,
                            "Frigobar",
                        );
                    });
                    settings_field(
                        &mut columns[1],
                        "Quantidade em estoque",
                        &mut self.admin_form.stock_quantity,
                        "Em branco = sem limite",
                    );
                    columns[0].add_space(8.0);
                    columns[0].label(RichText::new("Descrição").size(12.0).strong().color(INK));
                    columns[0].add(
                        egui::TextEdit::multiline(&mut self.admin_form.description).desired_rows(3),
                    );
                    columns[1].add_space(8.0);
                    columns[1].label(
                        RichText::new("Tom da garrafa")
                            .size(12.0)
                            .strong()
                            .color(INK),
                    );
                    egui::ComboBox::from_id_salt("settings-product-tone")
                        .selected_text(tone_label(&self.admin_form.tone))
                        .show_ui(&mut columns[1], |ui| {
                            for (value, label) in
                                [("ivory", "Clara"), ("gold", "Dourada"), ("olive", "Oliva")]
                            {
                                ui.selectable_value(
                                    &mut self.admin_form.tone,
                                    value.to_owned(),
                                    label,
                                );
                            }
                        });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.image_upload_busy,
                            egui::Button::new(if self.image_upload_busy {
                                "Enviando imagem…"
                            } else {
                                "Anexar imagem do computador"
                            }),
                        )
                        .clicked()
                    {
                        self.choose_product_image();
                    }
                    ui.label(
                        RichText::new(
                            self.admin_form
                                .image_url
                                .rsplit('/')
                                .next()
                                .filter(|s| !s.is_empty())
                                .unwrap_or("Nenhuma imagem selecionada"),
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                });
                ui.label(
                    RichText::new(
                        "PNG, JPEG e WebP · arquivos PNG preservam transparência · FiveManage",
                    )
                    .size(10.0)
                    .color(MUTED),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancelar").clicked() {
                        self.product_form_open = false;
                        self.editing_product_id = None;
                    }
                    let save = ui.add_enabled(
                        !self.image_upload_busy,
                        egui::Button::new(
                            RichText::new(if self.editing_product_id.is_some() {
                                "Salvar produto"
                            } else {
                                "Criar produto"
                            })
                            .color(Color32::WHITE)
                            .strong(),
                        )
                        .fill(GREEN)
                        .corner_radius(18),
                    );
                    if save.clicked() {
                        self.save_product();
                    }
                });
            });
        let _ = ctx;
    }

    fn admin_product_row(&mut self, ui: &mut egui::Ui, product: &Product) {
        Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, LINE))
            .corner_radius(11)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&product.label).size(13.0).strong().color(INK));
                        let promo = product
                            .promotional_price
                            .as_ref()
                            .map(|price| format!(" · Promoção {price}"))
                            .unwrap_or_default();
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {}{}",
                                if product.category == "frigobar" {
                                    "Frigobar"
                                } else {
                                    "Adega"
                                },
                                product.price,
                                product
                                    .stock_quantity
                                    .map(|q| format!("{q} un."))
                                    .unwrap_or_else(|| "Estoque livre".to_owned()),
                                promo
                            ))
                            .size(10.0)
                            .color(MUTED),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Excluir").clicked() {
                            self.delete_product(product.id.clone());
                        }
                        if ui.button("Editar").clicked() {
                            self.fill_product_form(product);
                        }
                    });
                });
            });
        ui.add_space(6.0);
    }

    fn authenticate_admin(&mut self, ctx: egui::Context) {
        self.admin_busy = true;
        self.admin_error = None;
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = reqwest::blocking::Client::new()
                .post(format!("{}/admin-session", api::API_BASE))
                .bearer_auth(token)
                .send()
                .and_then(|response| response.error_for_status().map(|_| ()))
                .map_err(|error| error.to_string());
            let _ = tx.send(Event::AdminAuthorized(result));
            ctx.request_repaint();
        });
    }

    fn reset_product_form(&mut self) {
        self.admin_form = ProductForm {
            category: CategoryFilter::Adega,
            tone: "ivory".to_owned(),
            ..Default::default()
        };
        self.editing_product_id = None;
        self.product_form_open = true;
        self.admin_error = None;
    }

    fn fill_product_form(&mut self, product: &Product) {
        self.admin_form = ProductForm {
            name: product.name.clone(),
            label: product.label.clone(),
            price: product.price.clone(),
            promotional_price: product.promotional_price.clone().unwrap_or_default(),
            stock_quantity: product
                .stock_quantity
                .map(|value| value.to_string())
                .unwrap_or_default(),
            description: product.description.clone(),
            image_url: product.image_url.clone().unwrap_or_default(),
            category: if product.category == "frigobar" {
                CategoryFilter::Frigobar
            } else {
                CategoryFilter::Adega
            },
            tone: product.tone.clone(),
        };
        self.editing_product_id = Some(product.id.clone());
        self.product_form_open = true;
        self.admin_error = None;
    }

    fn save_product(&mut self) {
        if self.admin_form.name.trim().is_empty()
            || self.admin_form.label.trim().is_empty()
            || self.admin_form.price.trim().is_empty()
        {
            self.admin_error = Some("Preencha nome, título e preço do produto.".to_owned());
            return;
        }
        let payload = serde_json::json!({
            "name": self.admin_form.name.trim(),
            "label": self.admin_form.label.trim(),
            "price": self.admin_form.price.trim(),
            "promotionalPrice": self.admin_form.promotional_price.trim(),
            "stockQuantity": self.admin_form.stock_quantity.trim().parse::<u32>().ok(),
            "description": self.admin_form.description.trim(),
            "imageUrl": self.admin_form.image_url.trim(),
            "category": if self.admin_form.category == CategoryFilter::Frigobar { "frigobar" } else { "adega" },
            "tone": if self.admin_form.tone.is_empty() { "ivory" } else { self.admin_form.tone.as_str() },
        });
        let id = self.editing_product_id.clone();
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        thread::spawn(move || {
            let endpoint = id
                .as_ref()
                .map(|id| format!("{}/products?id={}", api::API_BASE, urlencoding::encode(id)))
                .unwrap_or_else(|| format!("{}/products", api::API_BASE));
            let client = reqwest::blocking::Client::new();
            let request = if id.is_some() {
                client.put(endpoint)
            } else {
                client.post(endpoint)
            };
            let result = request
                .bearer_auth(token)
                .json(&payload)
                .send()
                .and_then(|response| response.error_for_status()?.json::<Product>())
                .map_err(|error| error.to_string());
            let _ = tx.send(Event::ProductSaved(result));
            ctx.request_repaint();
        });
    }

    fn delete_product(&self, id: String) {
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        thread::spawn(move || {
            let result = reqwest::blocking::Client::new()
                .delete(format!(
                    "{}/products?id={}",
                    api::API_BASE,
                    urlencoding::encode(&id)
                ))
                .bearer_auth(token)
                .send()
                .and_then(|response| response.error_for_status().map(|_| id))
                .map_err(|error| error.to_string());
            let _ = tx.send(Event::ProductDeleted(result));
            ctx.request_repaint();
        });
    }

    fn choose_product_image(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagem do produto", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg")
            .to_lowercase();
        let mime = match extension.as_str() {
            "png" => "image/png",
            "webp" => "image/webp",
            "jpg" | "jpeg" => "image/jpeg",
            _ => {
                self.admin_error = Some("Formato não suportado. Use PNG, JPEG ou WebP.".to_owned());
                return;
            }
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) if bytes.len() <= 1_700_000 => bytes,
            Ok(_) => {
                self.admin_error = Some("A imagem precisa ter até 1,7 MB.".to_owned());
                return;
            }
            Err(error) => {
                self.admin_error = Some(error.to_string());
                return;
            }
        };
        let data_url = format!("data:{mime};base64,{}", BASE64.encode(bytes));
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("produto.png")
            .to_owned();
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        self.image_upload_busy = true;
        thread::spawn(move || {
            let result = reqwest::blocking::Client::new()
                .post(format!("{}/product-image", api::API_BASE))
                .bearer_auth(token)
                .json(&serde_json::json!({"fileName":file_name,"dataUrl":data_url}))
                .send()
                .and_then(|response| response.error_for_status()?.json::<serde_json::Value>())
                .map_err(|error| error.to_string())
                .and_then(|value| {
                    value
                        .get("imageUrl")
                        .and_then(|url| url.as_str())
                        .map(str::to_owned)
                        .ok_or_else(|| "O servidor não retornou a URL da imagem.".to_owned())
                });
            let _ = tx.send(Event::ProductImageUploaded(result));
            ctx.request_repaint();
        });
    }

    fn settings_appearance(&mut self, ui: &mut egui::Ui) {
        settings_heading(
            ui,
            "Aparência e navegação",
            "Ajuste o acabamento visual do aplicativo desktop.",
        );
        ui.add_space(14.0);
        ui.checkbox(
            &mut self.preferences.glass_effect,
            "Efeito de vidro fosco na barra de navegação",
        );
        ui.label(RichText::new("Usa transparência suave, superfície clara e contorno para destacar o menu flutuante.").size(11.0).color(MUTED));
        ui.add_space(12.0);
        ui.add_space(16.0);
        Frame::new().fill(SURFACE_ALT).corner_radius(12).inner_margin(14.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::image(ui, &self.icons, Icon::Settings, 20.0, GREEN);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Tema da Matto Chalés").strong().color(INK));
                    ui.label(RichText::new("Verde pinho, tons de pedra e detalhes naturais inspirados na identidade da marca.").size(11.0).color(MUTED));
                });
            });
        });
    }

    fn settings_data(&mut self, ui: &mut egui::Ui) {
        settings_heading(
            ui,
            "Dados e conexão",
            "Confira onde as preferências ficam salvas e o estado dos serviços.",
        );
        ui.add_space(14.0);
        settings_status(
            ui,
            "Preferências do aplicativo",
            "Armazenadas neste computador",
        );
        settings_status(ui, "Disponibilidade", "Arquivo local · não sincronizado");
        settings_status(
            ui,
            "Catálogo de produtos",
            if self.product_error.is_some() {
                "API sem conexão no momento"
            } else if self.products_loading {
                "Sincronização em andamento"
            } else {
                "Catálogo carregado"
            },
        );
        ui.add_space(12.0);
        let path = self
            .preferences_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Caminho indisponível".to_owned());
        ui.label(
            RichText::new("Arquivo de preferências")
                .size(12.0)
                .strong()
                .color(INK),
        );
        ui.label(RichText::new(path).size(10.0).color(MUTED));
        ui.add_space(14.0);
        if ui.button("Restaurar preferências padrão").clicked() {
            self.preferences = AppPreferences::default();
            self.settings_notice =
                Some("Preferências padrão restauradas. Salve para aplicar.".to_owned());
        }
    }

    fn save_preferences(&mut self) {
        let Some(path) = &self.preferences_path else {
            self.settings_notice =
                Some("Não foi possível localizar a pasta de dados do aplicativo.".to_owned());
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match serde_json::to_vec_pretty(&self.preferences)
            .map_err(|error| error.to_string())
            .and_then(|data| fs::write(path, data).map_err(|error| error.to_string()))
        {
            Ok(()) => {
                self.settings_notice = Some("Configurações salvas neste computador.".to_owned())
            }
            Err(_) => {
                self.settings_notice = Some("Não foi possível salvar as configurações.".to_owned())
            }
        }
    }

    fn metric_card(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        amount: &str,
        detail: &str,
        color: Color32,
        width: f32,
    ) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 92.0), Sense::hover());
        ui.painter().rect_filled(rect, 18.0, SURFACE);
        ui.painter().rect_stroke(
            rect,
            18.0,
            Stroke::new(1.0, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            Pos2::new(rect.left() + 20.0, rect.top() + 24.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(14.0),
            MUTED,
        );
        let amount_pos = Pos2::new(rect.left() + 20.0, rect.top() + 61.0);
        ui.painter().text(
            amount_pos,
            Align2::LEFT_CENTER,
            amount,
            FontId::proportional(24.0),
            color,
        );
        ui.painter().text(
            Pos2::new(amount_pos.x + 0.5, amount_pos.y),
            Align2::LEFT_CENTER,
            amount,
            FontId::proportional(24.0),
            color,
        );
        if !detail.is_empty() {
            ui.painter().text(
                Pos2::new(rect.right() - 16.0, rect.center().y),
                Align2::RIGHT_CENTER,
                detail,
                FontId::proportional(12.0),
                MUTED,
            );
        }
    }

    fn finance_row(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        icon_bg: Color32,
        title: &str,
        detail: &str,
        amount: &str,
        income: bool,
    ) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 66.0), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 10.0, SURFACE_ALT);
        }
        let icon_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 4.0, rect.top() + 7.0),
            Vec2::splat(50.0),
        );
        ui.painter().rect_filled(icon_rect, 10.0, icon_bg);
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(icon_rect.center(), Vec2::splat(23.0)),
            if income { GREEN } else { ORANGE },
        );
        ui.painter().text(
            Pos2::new(rect.left() + 70.0, rect.center().y - 8.0),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(16.0),
            INK,
        );
        ui.painter().text(
            Pos2::new(rect.left() + 70.0, rect.center().y + 12.0),
            Align2::LEFT_CENTER,
            detail,
            FontId::proportional(12.0),
            MUTED,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 40.0, rect.center().y),
            Align2::RIGHT_CENTER,
            amount,
            FontId::proportional(15.0),
            INK,
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            Icon::ChevronRight,
            Rect::from_center_size(
                Pos2::new(rect.right() - 18.0, rect.center().y),
                Vec2::splat(18.0),
            ),
            MUTED,
        );
    }

    fn show_adega(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Adega").size(34.0).strong().color(INK));
                    ui.add_space(20.0);
                    Frame::new()
                        .fill(SURFACE)
                        .stroke(Stroke::new(1.0, Color32::WHITE))
                        .corner_radius(20)
                        .inner_margin(Margin::symmetric(13, 5))
                        .show(ui, |ui| {
                            ui.horizontal_centered(|ui| {
                                icons::image(ui, &self.icons, Icon::Search, 18.0, MUTED);
                                ui.add_sized(
                                    Vec2::new(245.0, 27.0),
                                    egui::TextEdit::singleline(&mut self.product_query)
                                        .hint_text("Buscar rótulo"),
                                );
                            });
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.text_button(ui, Icon::Refresh, "Atualizar", 120.0, 38.0) {
                            self.products_loading = true;
                            self.request_products(ctx.clone());
                        }
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!("{} produtos", self.products.len()))
                                .size(13.0)
                                .color(MUTED),
                        );
                    });
                });
                ui.add_space(20.0);

                if self.products_loading {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Carregando produtos...").color(MUTED));
                    });
                    return;
                }
                if let Some(error) = &self.product_error {
                    ui.label(RichText::new(error).color(RED));
                    return;
                }

                let query = self.product_query.to_lowercase();
                let products: Vec<Product> = self
                    .products
                    .iter()
                    .filter(|product| {
                        query.is_empty()
                            || product.name.to_lowercase().contains(&query)
                            || product.label.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect();

                Frame::new()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0, Color32::WHITE))
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                Vec2::new(72.0, 26.0),
                                egui::Label::new(RichText::new("Rótulo").strong().color(MUTED)),
                            );
                            ui.add_sized(
                                Vec2::new(440.0, 26.0),
                                egui::Label::new(RichText::new("Produto").strong().color(MUTED)),
                            );
                            ui.add_sized(
                                Vec2::new(190.0, 26.0),
                                egui::Label::new(RichText::new("Estoque").strong().color(MUTED)),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_sized(
                                    Vec2::new(150.0, 26.0),
                                    egui::Label::new(RichText::new("Preço").strong().color(MUTED)),
                                );
                            });
                        });
                        ui.separator();
                        for product in products {
                            self.product_row(ui, &product);
                            ui.separator();
                        }
                        if self.products.is_empty() {
                            ui.add_space(25.0);
                            ui.vertical_centered(|ui| {
                                icons::image(ui, &self.icons, Icon::Wine, 34.0, GREEN_MID);
                                ui.label(
                                    RichText::new("Nenhum produto cadastrado")
                                        .size(16.0)
                                        .strong()
                                        .color(INK),
                                );
                                ui.label(
                                    RichText::new("Os itens da adega aparecerão aqui.")
                                        .size(13.0)
                                        .color(MUTED),
                                );
                            });
                            ui.add_space(25.0);
                        }
                    });

                if let Some(selected_id) = &self.selected_product {
                    if let Some(product) = self.products.iter().find(|item| &item.id == selected_id)
                    {
                        ui.add_space(12.0);
                        Frame::new()
                            .fill(SURFACE_ALT)
                            .corner_radius(14)
                            .inner_margin(Margin::symmetric(18, 12))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&product.label).size(15.0).strong().color(INK),
                                );
                                ui.label(RichText::new(&product.description).color(MUTED));
                            });
                    }
                }
            });
    }

    fn product_row(&mut self, ui: &mut egui::Ui, product: &Product) {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 62.0), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 8.0, SURFACE_ALT);
        }
        let image_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + 6.0),
            Vec2::new(50.0, 50.0),
        );
        if let Some(texture) = self.product_images.get(&product.id) {
            ui.painter().image(
                texture.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui.painter().rect_filled(image_rect, 9.0, GREEN_PALE);
            icons::paint_in_rect(
                ui,
                &self.icons,
                Icon::Wine,
                Rect::from_center_size(image_rect.center(), Vec2::splat(24.0)),
                GREEN,
            );
        }
        let x = rect.left() + 76.0;
        ui.painter().text(
            Pos2::new(x, rect.center().y - 5.0),
            Align2::LEFT_CENTER,
            &product.label,
            FontId::proportional(15.0),
            INK,
        );
        ui.painter().text(
            Pos2::new(x + 300.0, rect.center().y),
            Align2::LEFT_CENTER,
            format!("{} garrafas", product.stock_quantity.unwrap_or(0)),
            FontId::proportional(14.0),
            MUTED,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 68.0, rect.center().y),
            Align2::RIGHT_CENTER,
            &product.price,
            FontId::proportional(15.0),
            INK,
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            Icon::ChevronRight,
            Rect::from_center_size(
                Pos2::new(rect.right() - 14.0, rect.center().y),
                Vec2::splat(17.0),
            ),
            MUTED,
        );
        if response.clicked() {
            self.selected_product = Some(product.id.clone());
        }
    }

    fn icon_button(
        &mut self,
        ui: &mut egui::Ui,
        icon: Icon,
        size: f32,
        fill: Color32,
        tint: Color32,
        label: &str,
    ) -> bool {
        let response = ui.add_sized(
            Vec2::splat(size),
            egui::Button::new("")
                .fill(fill)
                .stroke(Stroke::new(1.0, Color32::WHITE))
                .corner_radius(size / 2.0),
        );
        response.clone().on_hover_text(label);
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(response.rect.center(), Vec2::splat(size * 0.48)),
            tint,
        );
        if label == "Mes anterior" && response.clicked() {
            self.previous_month();
        }
        if label == "Proximo mes" && response.clicked() {
            self.next_month();
        }
        response.clicked()
    }

    fn action_button(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        fill: Color32,
        color: Color32,
        width: f32,
        action: fn(&mut Self),
    ) {
        let response = ui.add_sized(
            Vec2::new(width, 42.0),
            egui::Button::new(RichText::new(label).size(16.0).strong().color(color))
                .fill(fill)
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(170)))
                .corner_radius(21),
        );
        if response.clicked() {
            action(self);
        }
    }

    fn primary_button(&self, ui: &mut egui::Ui, label: &str, fill: Color32) -> bool {
        ui.add_sized(
            Vec2::new(ui.available_width(), 46.0),
            egui::Button::new(
                RichText::new(label)
                    .size(16.0)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(23),
        )
        .clicked()
    }

    fn text_button(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        label: &str,
        width: f32,
        height: f32,
    ) -> bool {
        let response = ui.add_sized(
            Vec2::new(width, height),
            egui::Button::new("")
                .fill(SURFACE)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(height * 0.5),
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            Rect::from_center_size(
                Pos2::new(response.rect.left() + 24.0, response.rect.center().y),
                Vec2::splat(16.0),
            ),
            INK,
        );
        ui.painter().text(
            Pos2::new(response.rect.left() + 40.0, response.rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            INK,
        );
        response.clicked()
    }

    fn legend(&self, ui: &mut egui::Ui, color: Color32, label: &str) {
        ui.horizontal_centered(|ui| {
            ui.painter().circle_filled(
                Pos2::new(ui.cursor().left() + 6.0, ui.cursor().center().y),
                6.0,
                color,
            );
            ui.add_space(16.0);
            ui.label(RichText::new(label).size(12.0).color(MUTED));
        });
    }

    fn select_day(&mut self, day: u32) {
        self.selected_range = match self.selected_range {
            Some((start, end)) if start != end => Some((day, day)),
            Some((start, _)) => Some((start.min(day), start.max(day))),
            None => Some((day, day)),
        };
        self.notice = None;
    }

    fn day_selected(&self, day: u32) -> bool {
        self.selected_range
            .is_some_and(|(start, end)| (start..=end).contains(&day))
    }

    fn day_status(&self, day: u32) -> DayStatus {
        let key = date_key(self.year, self.month, day);
        if self.occupied_dates.contains(&key) {
            return DayStatus::Ocupada;
        }
        if self.blocked_dates.contains(&key) {
            DayStatus::Bloqueada
        } else {
            DayStatus::Disponivel
        }
    }

    fn range_label(&self) -> String {
        match self.selected_range {
            Some((start, end)) if start == end => {
                format!("{} de {}", start, month_name(self.month).to_lowercase())
            }
            Some((start, end)) => format!(
                "{} a {} de {}",
                start,
                end,
                month_name(self.month).to_lowercase()
            ),
            None => "Selecione as datas no calendário".to_owned(),
        }
    }

    fn block_selected(&mut self) {
        self.apply_block_state(true);
    }

    fn release_selected(&mut self) {
        self.apply_block_state(false);
    }

    fn apply_block_state(&mut self, blocked: bool) {
        if let Some((start, end)) = self.selected_range {
            for day in start..=end {
                let key = date_key(self.year, self.month, day);
                if blocked {
                    self.blocked_dates.insert(key);
                } else {
                    self.blocked_dates.remove(&key);
                }
            }
            self.notice = Some(if blocked {
                "Datas marcadas para bloqueio. Salve para aplicar.".to_owned()
            } else {
                "Datas liberadas. Salve para aplicar.".to_owned()
            });
        } else {
            self.notice = Some("Selecione um período no calendário.".to_owned());
        }
    }

    fn save_availability(&mut self) {
        if let Some(path) = &self.availability_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut dates: Vec<_> = self.blocked_dates.iter().cloned().collect();
            dates.sort();
            match fs::write(path, dates.join("\n")) {
                Ok(()) => self.notice = Some("Disponibilidade salva neste computador.".to_owned()),
                Err(_) => {
                    self.notice = Some("Não foi possível salvar a disponibilidade.".to_owned())
                }
            }
        } else {
            self.notice =
                Some("Não foi possível localizar a pasta de dados do aplicativo.".to_owned());
        }
    }

    fn previous_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
        self.selected_range = None;
    }

    fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
        self.selected_range = None;
    }

    fn advance_profile_animation(&mut self, ctx: &egui::Context) {
        if self.profile_frames.len() < 2 {
            return;
        }
        let now = Instant::now();
        if now >= self.next_profile_frame {
            self.profile_frame_index = (self.profile_frame_index + 1) % self.profile_frames.len();
            self.next_profile_frame = now + self.profile_frames[self.profile_frame_index].delay;
        }
        ctx.request_repaint_after(self.next_profile_frame.saturating_duration_since(now));
    }
}

impl eframe::App for DashboardApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.last_context = ctx.clone();
        self.advance_profile_animation(&ctx);
        self.handle_events(&ctx);
        self.title_bar(root);
        if self.section == Section::Launcher {
            self.show_launcher(root);
            return;
        }
        self.navigation(root);
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(CANVAS)
                    .inner_margin(Margin::symmetric(26, 14)),
            )
            .show(root, |ui| match self.section {
                Section::Launcher => {}
                Section::Reservas => self.show_reservas(ui),
                Section::Financas => self.show_financas(ui),
                Section::Adega => self.show_adega(ui, &ctx),
                Section::Configuracoes => self.show_settings(ui, &ctx),
            });
    }
}

fn set_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = Color32::from_rgba_unmultiplied(250, 249, 244, 242);
    style.visuals.extreme_bg_color = SURFACE;
    style.visuals.faint_bg_color = SURFACE_ALT;
    style.visuals.override_text_color = Some(INK);
    style.visuals.selection.bg_fill = GREEN_PALE;
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.fg_stroke.color = INK;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.hovered.bg_fill = GREEN_PALE;
    style.visuals.widgets.hovered.fg_stroke.color = INK;
    style.visuals.widgets.active.bg_fill = GREEN;
    style.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(13.0, 8.0);
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(ctx.load_texture(
        name.to_owned(),
        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    ))
}

fn load_availability() -> (HashSet<String>, Option<PathBuf>) {
    let Some(project_dirs) = directories::ProjectDirs::from("br", "Matto Chales", "Matto Chales")
    else {
        return (HashSet::new(), None);
    };
    let path = project_dirs.data_local_dir().join("datas-bloqueadas.txt");
    let blocked = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    (blocked, Some(path))
}

fn load_preferences() -> (AppPreferences, Option<PathBuf>) {
    let Some(project_dirs) = directories::ProjectDirs::from("br", "Matto Chales", "Matto Chales")
    else {
        return (AppPreferences::default(), None);
    };
    let path = project_dirs.config_local_dir().join("configuracoes.json");
    let preferences = fs::read(&path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default();
    (preferences, Some(path))
}

fn load_user_profile(ctx: &egui::Context) -> (String, Vec<ProfileFrame>) {
    let Some(project_dirs) = directories::ProjectDirs::from("br", "Matto Chales", "Matto Chales")
    else {
        return (String::new(), Vec::new());
    };
    let path = project_dirs.config_local_dir().join("perfil.json");
    let profile: UserProfile = fs::read(&path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default();
    let Some(photo_path) = profile.photo_path.as_deref() else {
        return (profile.name, Vec::new());
    };
    let Ok(bytes) = fs::read(photo_path) else {
        return (profile.name, Vec::new());
    };

    let frames = if PathBuf::from(photo_path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
    {
        image::codecs::gif::GifDecoder::new(Cursor::new(bytes.as_slice()))
            .ok()
            .and_then(|decoder| image::AnimationDecoder::into_frames(decoder).collect_frames().ok())
            .unwrap_or_default()
            .into_iter()
            .take(120)
            .enumerate()
            .map(|(index, frame)| {
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                let delay_ms = if denominator == 0 {
                    100
                } else {
                    (numerator / denominator).max(40)
                };
                let image = image::DynamicImage::ImageRgba8(frame.into_buffer())
                    .thumbnail(96, 96)
                    .to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                ProfileFrame {
                    texture: ctx.load_texture(
                        format!("matto-user-profile-{index}"),
                        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                        egui::TextureOptions::LINEAR,
                    ),
                    delay: Duration::from_millis(u64::from(delay_ms)),
                }
            })
            .collect()
    } else {
        image::load_from_memory(&bytes)
            .ok()
            .map(|image| {
                let image = image.thumbnail(96, 96).to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                vec![ProfileFrame {
                    texture: ctx.load_texture(
                        "matto-user-profile-static",
                        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                        egui::TextureOptions::LINEAR,
                    ),
                    delay: Duration::from_secs(1),
                }]
            })
            .unwrap_or_default()
    };

    (profile.name, frames)
}

fn settings_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(21.0).strong().color(INK));
    ui.label(RichText::new(subtitle).size(12.0).color(MUTED));
    ui.add_space(6.0);
    ui.separator();
}

fn settings_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.add_space(7.0);
    ui.label(RichText::new(label).size(12.0).strong().color(INK));
    ui.add_sized(
        Vec2::new(ui.available_width(), 38.0),
        egui::TextEdit::singleline(value).hint_text(hint),
    );
}

fn settings_status(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.0).strong().color(INK));
        });
    });
    ui.separator();
}

fn tone_label(tone: &str) -> &'static str {
    match tone {
        "gold" => "Dourada",
        "olive" => "Oliva",
        _ => "Clara",
    }
}

fn date_key(year: i32, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Janeiro",
        2 => "Fevereiro",
        3 => "Março",
        4 => "Abril",
        5 => "Maio",
        6 => "Junho",
        7 => "Julho",
        8 => "Agosto",
        9 => "Setembro",
        10 => "Outubro",
        11 => "Novembro",
        _ => "Dezembro",
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 30,
    }
}

fn weekday_monday(year: i32, month: u32) -> usize {
    let mut y = year;
    let mut m = month as i32;
    if m < 3 {
        y -= 1;
        m += 12;
    }
    let k = y % 100;
    let j = y / 100;
    let saturday_zero = (1 + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    ((saturday_zero + 5) % 7) as usize
}
