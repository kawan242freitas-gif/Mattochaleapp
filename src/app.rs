use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

use eframe::egui::{self, Align, Color32, FontId, Frame, Layout, Margin, RichText, Stroke, Vec2};

use crate::{
    api,
    icons::{self, Icon},
    model::{CartEntry, CategoryFilter, Page, PersistedState, Product},
};

const BG: Color32 = Color32::from_rgb(17, 13, 10);
const PANEL: Color32 = Color32::from_rgb(27, 20, 15);
const PANEL_HI: Color32 = Color32::from_rgb(37, 27, 20);
const BORDER: Color32 = Color32::from_rgb(76, 56, 39);
const GOLD: Color32 = Color32::from_rgb(189, 137, 83);
const TEXT: Color32 = Color32::from_rgb(243, 236, 227);
const MUTED: Color32 = Color32::from_rgb(178, 164, 149);
const GREEN: Color32 = Color32::from_rgb(190, 118, 55);

enum Event {
    Products(Result<Vec<Product>, String>),
    Image(String, Result<image::RgbaImage, String>),
    OrderCreated(Result<api::CreatedOrderResponse, String>),
    ProofSubmitted(Result<(), String>),
    AdminAuthorized(Result<(), String>),
    ProductSaved(Result<Product, String>),
    ProductDeleted(Result<String, String>),
    ProductImageUploaded(Result<String, String>),
}

pub struct MattoApp {
    page: Page,
    products: Vec<Product>,
    cart: Vec<CartEntry>,
    query: String,
    category: CategoryFilter,
    selected_product: Option<String>,
    textures: HashMap<String, egui::TextureHandle>,
    icons: HashMap<Icon, egui::TextureHandle>,
    brand_texture: Option<egui::TextureHandle>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    products_loading: bool,
    error: Option<String>,
    notice: Option<String>,
    state_path: Option<PathBuf>,
    hero_texture: Option<egui::TextureHandle>,
    hero_loading: bool,
    customer_name: String,
    chalet_number: String,
    checkout_busy: bool,
    order_token: Option<String>,
    order_number: Option<String>,
    order_total: f64,
    receipt_busy: bool,
    receipt_path: Option<PathBuf>,
    receipt_preview: Option<egui::TextureHandle>,
    admin_token: String,
    admin_unlocked: bool,
    admin_busy: bool,
    admin_form: ProductForm,
    editing_id: Option<String>,
    admin_filter: CategoryFilter,
    admin_form_open: bool,
    product_image_upload_busy: bool,
    last_context: egui::Context,
}

#[derive(Default)]
struct ProductForm {
    name: String,
    label: String,
    price: String,
    promo_price: String,
    quantity: String,
    description: String,
    image_url: String,
    category: CategoryFilter,
    tone: String,
}

impl MattoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        set_theme(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let (cart, state_path) = load_state();
        let mut app = Self {
            page: Page::Home,
            products: vec![],
            cart,
            query: String::new(),
            category: CategoryFilter::Todos,
            selected_product: None,
            textures: HashMap::new(),
            icons: icons::load(&cc.egui_ctx),
            brand_texture: image::load_from_memory(include_bytes!(
                "../../assets/matto-chales-cabana.png"
            ))
            .ok()
            .map(|image| {
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                cc.egui_ctx.load_texture(
                    "matto-chales-brand",
                    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    egui::TextureOptions::LINEAR,
                )
            }),
            tx,
            rx,
            products_loading: true,
            error: None,
            notice: None,
            state_path,
            hero_texture: None,
            hero_loading: false,
            customer_name: String::new(),
            chalet_number: String::new(),
            checkout_busy: false,
            order_token: None,
            order_number: None,
            order_total: 0.0,
            receipt_busy: false,
            receipt_path: None,
            receipt_preview: None,
            admin_token: String::new(),
            admin_unlocked: false,
            admin_busy: false,
            admin_form: ProductForm::default(),
            editing_id: None,
            admin_filter: CategoryFilter::Todos,
            admin_form_open: false,
            product_image_upload_busy: false,
            last_context: cc.egui_ctx.clone(),
        };
        app.refresh_products(&cc.egui_ctx);
        app.load_hero(&cc.egui_ctx);
        app
    }

    fn refresh_products(&mut self, ctx: &egui::Context) {
        self.products_loading = true;
        self.error = None;
        let tx = self.tx.clone();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let result = api::fetch_products().map_err(|e| e.to_string());
            let _ = tx.send(Event::Products(result));
            repaint.request_repaint();
        });
    }

    fn load_hero(&mut self, ctx: &egui::Context) {
        if self.hero_loading || self.hero_texture.is_some() {
            return;
        }
        self.hero_loading = true;
        let tx = self.tx.clone();
        let repaint = ctx.clone();
        thread::spawn(move || {
            let result = api::fetch_image(api::HERO_IMAGE_URL).map_err(|e| e.to_string());
            let _ = tx.send(Event::Image("__hero".into(), result));
            repaint.request_repaint();
        });
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Products(result) => match result {
                    Ok(products) => {
                        self.products_loading = false;
                        self.error = None;
                        for product in &products {
                            if self.textures.contains_key(&product.id) {
                                continue;
                            }
                            if let Some(url) = product.image_url.clone() {
                                let tx = self.tx.clone();
                                let repaint = ctx.clone();
                                let id = product.id.clone();
                                thread::spawn(move || {
                                    let result = api::fetch_image(&url).map_err(|e| e.to_string());
                                    let _ = tx.send(Event::Image(id, result));
                                    repaint.request_repaint();
                                });
                            }
                        }
                        self.products = products;
                    }
                    Err(error) => {
                        self.products_loading = false;
                        self.error = Some(error);
                    }
                },
                Event::Image(id, result) => match result {
                    Ok(image) => {
                        let size = [image.width() as usize, image.height() as usize];
                        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                        let texture = ctx.load_texture(
                            format!("product-{id}"),
                            color,
                            egui::TextureOptions::LINEAR,
                        );
                        if id == "__hero" {
                            self.hero_texture = Some(texture);
                        } else {
                            self.textures.insert(id, texture);
                        }
                    }
                    Err(message) if id == "__hero" => {
                        self.notice = Some(format!("A imagem de destaque não carregou: {message}"))
                    }
                    Err(_) => {}
                },
                Event::OrderCreated(result) => {
                    self.checkout_busy = false;
                    match result {
                        Ok(order) => {
                            self.order_token = Some(order.token);
                            self.order_number = order.order.and_then(|value| {
                                value
                                    .get("id")
                                    .and_then(|id| id.as_str())
                                    .map(str::to_owned)
                            });
                            self.order_total = self.subtotal();
                            self.notice =
                                Some("Comanda criada. Faça o Pix e anexe o comprovante.".into());
                        }
                        Err(error) => {
                            self.error = Some(format!("Não foi possível criar a comanda: {error}"))
                        }
                    }
                }
                Event::ProofSubmitted(result) => {
                    self.receipt_busy = false;
                    match result {
                        Ok(()) => {
                            let token = self.order_token.clone().unwrap_or_default();
                            let order_url = api::order_detail_url(&token);
                            let items = self
                                .cart
                                .iter()
                                .filter_map(|entry| {
                                    self.product(&entry.product_id).map(|product| {
                                        format!("{}x {}", entry.quantity, product.name)
                                    })
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            let message = format!(
                                "Olá! Segue meu pedido\nCliente: {}\nChalé: {}\nComprovante e itens do pedido: {}\nItens: {}\nTotal: {}",
                                self.customer_name,
                                self.chalet_number,
                                order_url,
                                items,
                                currency(self.order_total)
                            );
                            let whatsapp = format!(
                                "https://wa.me/{}?text={}",
                                "553598908891",
                                urlencoding::encode(&message)
                            );
                            let _ = open::that(whatsapp);
                            self.cart.clear();
                            self.save_state();
                            self.notice = Some(
                                "Comprovante enviado e WhatsApp aberto com o resumo do pedido."
                                    .into(),
                            );
                        }
                        Err(error) => {
                            self.error = Some(format!("Falha ao enviar comprovante: {error}"))
                        }
                    }
                }
                Event::AdminAuthorized(result) => {
                    self.admin_busy = false;
                    match result {
                        Ok(()) => {
                            self.admin_unlocked = true;
                            self.page = Page::Configuracoes;
                            self.notice = Some("Acesso administrativo autorizado.".into());
                        }
                        Err(error) => {
                            self.error = Some(format!("Não foi possível entrar: {error}"))
                        }
                    }
                }
                Event::ProductSaved(result) => match result {
                    Ok(product) => {
                        if let Some(index) =
                            self.products.iter().position(|item| item.id == product.id)
                        {
                            self.products[index] = product;
                        } else {
                            self.products.push(product);
                        }
                        self.notice = Some("Produto salvo no catálogo.".into());
                        self.reset_product_form();
                    }
                    Err(error) => {
                        self.error = Some(format!("Não foi possível salvar o produto: {error}"))
                    }
                },
                Event::ProductDeleted(result) => match result {
                    Ok(id) => {
                        self.products.retain(|product| product.id != id);
                        self.notice = Some("Produto removido do catálogo.".into());
                    }
                    Err(error) => {
                        self.error = Some(format!("Não foi possível remover o produto: {error}"))
                    }
                },
                Event::ProductImageUploaded(result) => {
                    self.product_image_upload_busy = false;
                    match result {
                        Ok(url) => {
                            self.admin_form.image_url = url;
                            self.notice = Some("Imagem enviada para o armazenamento.".into());
                        }
                        Err(error) => self.error = Some(format!("Falha ao enviar imagem: {error}")),
                    }
                }
            }
        }
    }

    fn save_state(&self) {
        let Some(path) = &self.state_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let state = PersistedState {
            cart: self.cart.clone(),
        };
        if let Ok(json) = serde_json::to_vec_pretty(&state) {
            let _ = fs::write(path, json);
        }
    }

    fn add_to_cart(&mut self, product_id: &str) {
        let entry = self
            .cart
            .iter_mut()
            .find(|item| item.product_id == product_id);
        if let Some(entry) = entry {
            entry.quantity = (entry.quantity + 1).min(99);
        } else {
            self.cart.push(CartEntry {
                product_id: product_id.to_owned(),
                quantity: 1,
            });
        }
        self.save_state();
        self.notice = Some("Produto adicionado ao carrinho".into());
    }

    fn change_quantity(&mut self, product_id: &str, delta: i32) {
        if let Some(index) = self
            .cart
            .iter()
            .position(|item| item.product_id == product_id)
        {
            let quantity = self.cart[index].quantity as i32 + delta;
            if quantity <= 0 {
                self.cart.remove(index);
            } else {
                self.cart[index].quantity = quantity.min(99) as u32;
            }
            self.save_state();
        }
    }

    fn product(&self, id: &str) -> Option<&Product> {
        self.products.iter().find(|item| item.id == id)
    }

    fn cart_count(&self) -> u32 {
        self.cart.iter().map(|entry| entry.quantity).sum()
    }

    fn subtotal(&self) -> f64 {
        self.cart
            .iter()
            .map(|entry| {
                self.product(&entry.product_id)
                    .map(|product| price(product) * entry.quantity as f64)
                    .unwrap_or(0.0)
            })
            .sum()
    }

    fn nav_item(&mut self, ui: &mut egui::Ui, page: Page, icon: Icon, label: &str) {
        let active = self.page == page;
        let fill = if active {
            PANEL_HI
        } else {
            Color32::TRANSPARENT
        };
        let text_color = if active { GOLD } else { MUTED };
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), egui::Sense::click());
        ui.painter().rect_filled(
            rect,
            10.0,
            if response.hovered() && !active {
                PANEL_HI
            } else {
                fill
            },
        );
        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 27.0, rect.center().y),
            Vec2::splat(19.0),
        );
        icons::paint_in_rect(ui, &self.icons, icon, icon_rect, text_color);
        ui.painter().text(
            egui::pos2(rect.left() + 52.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(15.0),
            text_color,
        );
        if page == Page::Carrinho && self.cart_count() > 0 {
            let badge = egui::pos2(rect.right() - 20.0, rect.center().y);
            ui.painter().circle_filled(badge, 9.0, GOLD);
            ui.painter().text(
                badge,
                egui::Align2::CENTER_CENTER,
                self.cart_count().to_string(),
                FontId::proportional(10.0),
                BG,
            );
        }
        if response.clicked() {
            self.page = page;
        }
    }

    fn lucide_text_button(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        label: &str,
        width: f32,
        height: f32,
        fill: Color32,
    ) -> egui::Response {
        let response = ui.add_sized(
            Vec2::new(width, height),
            egui::Button::new("")
                .fill(fill)
                .stroke(Stroke::NONE)
                .corner_radius(6),
        );
        let rect = response.rect;
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 22.0, rect.center().y),
                Vec2::splat(17.0),
            ),
            TEXT,
        );
        ui.painter().text(
            egui::pos2(rect.left() + 38.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(13.0),
            TEXT,
        );
        response
    }

    fn lucide_icon_button(
        &self,
        ui: &mut egui::Ui,
        icon: Icon,
        size: f32,
        fill: Color32,
        tint: Color32,
    ) -> egui::Response {
        let response = ui.add_sized(
            Vec2::splat(size),
            egui::Button::new("")
                .fill(fill)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(8),
        );
        icons::paint_in_rect(
            ui,
            &self.icons,
            icon,
            egui::Rect::from_center_size(response.rect.center(), Vec2::splat(size * 0.48)),
            tint,
        );
        response
    }

    fn page_header(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new(title).size(26.0).strong().color(TEXT));
                ui.label(RichText::new(subtitle).size(13.0).color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} produtos", self.products.len()))
                        .color(MUTED)
                        .size(13.0),
                );
            });
        });
        ui.add_space(18.0);
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (filter, label) in [
                (CategoryFilter::Todos, "Todos"),
                (CategoryFilter::Adega, "Adega"),
                (CategoryFilter::Frigobar, "Frigobar"),
            ] {
                let active = self.category == filter;
                if ui
                    .add(
                        egui::Button::new(RichText::new(label).size(13.0).color(if active {
                            TEXT
                        } else {
                            MUTED
                        }))
                        .fill(if active {
                            Color32::from_rgb(111, 74, 49)
                        } else {
                            PANEL
                        })
                        .stroke(Stroke::new(1.0, BORDER))
                        .corner_radius(18)
                        .min_size(Vec2::new(78.0, 34.0)),
                    )
                    .clicked()
                {
                    self.category = filter;
                }
            }
        });
    }

    fn filtered_products(&self) -> Vec<Product> {
        let needle = self.query.trim().to_lowercase();
        self.products
            .iter()
            .filter(|product| {
                let category_matches = self.category == CategoryFilter::Todos
                    || (self.category == CategoryFilter::Adega && product.category == "adega")
                    || (self.category == CategoryFilter::Frigobar
                        && product.category == "frigobar");
                let query_matches = needle.is_empty()
                    || product.name.to_lowercase().contains(&needle)
                    || product.label.to_lowercase().contains(&needle)
                    || product.description.to_lowercase().contains(&needle);
                category_matches && query_matches
            })
            .cloned()
            .collect()
    }

    fn product_card(&mut self, ui: &mut egui::Ui, product: &Product, width: f32) {
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(12)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.set_width(width - 30.0);
                ui.set_height(374.0);
                ui.vertical_centered(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(&product.label)
                                    .strong()
                                    .size(14.0)
                                    .color(TEXT),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(width - 32.0, 30.0)),
                        )
                        .clicked()
                    {
                        self.selected_product = Some(product.id.clone());
                        self.page = Page::Detalhe;
                    }
                    ui.add_space(5.0);
                    let photo_size = Vec2::new(width - 54.0, 185.0);
                    if let Some(texture) = self.textures.get(&product.id) {
                        ui.add(
                            egui::Image::new((texture.id(), photo_size))
                                .fit_to_exact_size(photo_size),
                        );
                    } else {
                        let (rect, _) = ui.allocate_exact_size(photo_size, egui::Sense::hover());
                        ui.painter().rect_filled(rect, 10.0, PANEL_HI);
                        icons::paint_in_rect(
                            ui,
                            &self.icons,
                            Icon::Wine,
                            egui::Rect::from_center_size(rect.center(), Vec2::splat(54.0)),
                            GOLD,
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&product.description)
                            .size(12.0)
                            .color(MUTED)
                            .line_height(Some(17.0)),
                    );
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        for _ in 0..5 {
                            icons::image(ui, &self.icons, Icon::Star, 13.0, GOLD);
                        }
                    });
                    ui.add_space(7.0);
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.label(
                            RichText::new(currency(price(product)))
                                .size(16.0)
                                .strong()
                                .color(GOLD),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self
                                .lucide_text_button(
                                    ui,
                                    Icon::ShoppingCart,
                                    "Adicionar ao carrinho",
                                    176.0,
                                    36.0,
                                    Color32::from_rgb(111, 74, 49),
                                )
                                .clicked()
                            {
                                self.add_to_cart(&product.id);
                            }
                        });
                    });
                });
            });
    }

    fn product_grid(&mut self, ui: &mut egui::Ui, products: Vec<Product>) {
        let cols = if ui.available_width() > 1120.0 { 4 } else { 3 };
        let width =
            ((ui.available_width() - (cols as f32 - 1.0) * 16.0) / cols as f32).clamp(235.0, 390.0);
        egui::Grid::new("product_grid")
            .num_columns(cols)
            .spacing(Vec2::new(16.0, 16.0))
            .show(ui, |ui| {
                for (index, product) in products.iter().enumerate() {
                    self.product_card(ui, product, width);
                    if (index + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn show_home(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(18)
            .inner_margin(Margin::same(28))
            .show(ui, |ui| {
                ui.set_min_height(350.0);
                ui.columns(2, |columns| {
                    columns[0].vertical_centered_justified(|ui| {
                        ui.add_space(44.0);
                        ui.label(
                            RichText::new("MATTO CHALÉS  ·  ADEGA E FRIGOBAR")
                                .size(12.0)
                                .strong()
                                .color(GOLD),
                        );
                        ui.add_space(18.0);
                        ui.heading(
                            RichText::new("SEJA BEM-VINDO\nÀ NOSSA ADEGA")
                                .size(43.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("AS MELHORES BEBIDAS ESTÃO AQUI")
                                .size(12.0)
                                .strong()
                                .color(MUTED),
                        );
                        ui.add_space(26.0);
                        if self
                            .lucide_text_button(
                                ui,
                                Icon::ArrowRight,
                                "Explorar produtos",
                                208.0,
                                44.0,
                                Color32::from_rgb(111, 74, 49),
                            )
                            .clicked()
                        {
                            self.page = Page::Buscar;
                        }
                    });
                    columns[1].vertical_centered(|ui| {
                        if let Some(texture) = &self.hero_texture {
                            let available = ui.available_width().min(490.0);
                            ui.add(
                                egui::Image::new((texture.id(), Vec2::new(available, 330.0)))
                                    .shrink_to_fit(),
                            );
                        } else {
                            ui.label(
                                RichText::new("Cestas e bebidas selecionadas para você")
                                    .color(MUTED),
                            );
                        }
                    });
                });
            });
        ui.add_space(28.0);
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("NOSSOS PRODUTOS")
                    .size(22.0)
                    .strong()
                    .color(TEXT),
            );
            ui.add_space(12.0);
            self.filter_bar(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self
                    .lucide_text_button(ui, Icon::Refresh, "Atualizar catálogo", 168.0, 34.0, PANEL)
                    .clicked()
                {
                    self.refresh_products(ctx);
                }
            });
        });
        ui.add_space(16.0);
        self.show_products(ui);
    }

    fn show_products(&mut self, ui: &mut egui::Ui) {
        if self.products_loading {
            ui.spinner();
            ui.label(RichText::new("Carregando produtos da adega…").color(MUTED));
        } else if let Some(error) = self.error.clone() {
            ui.label(RichText::new(error).color(Color32::from_rgb(228, 115, 103)));
            if self
                .lucide_text_button(ui, Icon::Refresh, "Tentar novamente", 158.0, 36.0, PANEL_HI)
                .clicked()
            {
                let ctx = self.last_context.clone();
                self.refresh_products(&ctx);
            }
        } else {
            let products = self.filtered_products();
            if products.is_empty() {
                ui.label(
                    RichText::new("Nenhum produto cadastrado nesta categoria ainda.").color(MUTED),
                );
            } else {
                self.product_grid(ui, products);
            }
        }
    }

    fn show_search(&mut self, ui: &mut egui::Ui) {
        self.page_header(
            ui,
            "Buscar produtos",
            "Encontre bebidas por nome, categoria ou descrição.",
        );
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(10)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icons::image(ui, &self.icons, Icon::Search, 21.0, GOLD);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Buscar na adega e no frigobar…")
                            .desired_width(ui.available_width()),
                    );
                });
            });
        ui.add_space(15.0);
        self.filter_bar(ui);
        ui.add_space(18.0);
        self.show_products(ui);
    }

    fn show_details(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected_product.clone() else {
            self.page = Page::Home;
            return;
        };
        let Some(product) = self.product(&id).cloned() else {
            self.page = Page::Home;
            return;
        };
        if self
            .lucide_text_button(
                ui,
                Icon::ChevronLeft,
                "Voltar aos produtos",
                190.0,
                36.0,
                PANEL,
            )
            .clicked()
        {
            self.page = Page::Home;
        }
        ui.add_space(18.0);
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(16)
            .inner_margin(28.0)
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].vertical_centered(|ui| {
                        ui.label(
                            RichText::new(&product.category.to_uppercase())
                                .strong()
                                .color(GOLD),
                        );
                        if let Some(texture) = self.textures.get(&id) {
                            ui.add(
                                egui::Image::new((texture.id(), Vec2::new(430.0, 440.0)))
                                    .shrink_to_fit(),
                            );
                        } else {
                            ui.add_space(380.0);
                        }
                    });
                    columns[1].vertical_centered_justified(|ui| {
                        ui.add_space(68.0);
                        ui.heading(
                            RichText::new(&product.label)
                                .size(34.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.add_space(14.0);
                        ui.heading(
                            RichText::new(currency(price(&product)))
                                .size(26.0)
                                .strong()
                                .color(GOLD),
                        );
                        ui.add_space(20.0);
                        ui.separator();
                        ui.label(
                            RichText::new("Sobre este produto")
                                .size(19.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(RichText::new(&product.description).size(15.0).color(MUTED));
                        ui.add_space(20.0);
                        if let Some(quantity) = product.stock_quantity {
                            ui.label(
                                RichText::new(format!("{} unidades disponíveis", quantity))
                                    .color(if quantity > 0 { GREEN } else { GOLD }),
                            );
                        }
                        ui.add_space(20.0);
                        if self
                            .lucide_text_button(
                                ui,
                                Icon::ShoppingCart,
                                "Adicionar ao carrinho",
                                230.0,
                                48.0,
                                Color32::from_rgb(111, 74, 49),
                            )
                            .clicked()
                        {
                            self.add_to_cart(&id);
                        }
                    });
                });
            });
    }

    fn show_cart(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui, "Carrinho", "Revise seu pedido antes de continuar.");
        if self.order_token.is_some() {
            self.show_pix_checkout(ui);
            return;
        }
        if self.cart.is_empty() {
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(14)
                .inner_margin(40.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        icons::image(ui, &self.icons, Icon::ShoppingBag, 40.0, GOLD);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Sua cesta está vazia")
                                .size(22.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Adicione produtos para montar seu pedido.").color(MUTED),
                        );
                        if self
                            .lucide_text_button(
                                ui,
                                Icon::ArrowRight,
                                "Explorar produtos",
                                194.0,
                                38.0,
                                PANEL_HI,
                            )
                            .clicked()
                        {
                            self.page = Page::Buscar;
                        }
                    });
                });
            return;
        }
        let cart = self.cart.clone();
        for item in cart {
            let product = self.product(&item.product_id).cloned();
            let Some(product) = product else {
                continue;
            };
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(11)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(texture) = self.textures.get(&product.id) {
                            ui.add(
                                egui::Image::new((texture.id(), Vec2::new(76.0, 86.0)))
                                    .shrink_to_fit(),
                            );
                        } else {
                            ui.allocate_space(Vec2::new(76.0, 86.0));
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&product.label).strong().color(TEXT));
                            ui.label(RichText::new(currency(price(&product))).color(GOLD));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self
                                .lucide_text_button(
                                    ui,
                                    Icon::Trash,
                                    "Remover",
                                    94.0,
                                    32.0,
                                    PANEL_HI,
                                )
                                .clicked()
                            {
                                self.change_quantity(&product.id, -(item.quantity as i32));
                            }
                            if self
                                .lucide_icon_button(ui, Icon::Plus, 31.0, PANEL_HI, GOLD)
                                .clicked()
                            {
                                self.change_quantity(&product.id, 1);
                            }
                            ui.label(RichText::new(item.quantity.to_string()).color(TEXT));
                            if self
                                .lucide_icon_button(ui, Icon::Minus, 31.0, PANEL_HI, GOLD)
                                .clicked()
                            {
                                self.change_quantity(&product.id, -1);
                            }
                        });
                    });
                });
            ui.add_space(8.0);
        }
        ui.add_space(16.0);
        Frame::new()
            .fill(PANEL_HI)
            .corner_radius(12)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Total do pedido")
                            .size(17.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(currency(self.subtotal()))
                                .size(22.0)
                                .strong()
                                .color(GOLD),
                        )
                    });
                });
            });
        ui.add_space(24.0);
        ui.heading(
            RichText::new("Finalizar pedido")
                .size(20.0)
                .strong()
                .color(TEXT),
        );
        ui.label(
            RichText::new("Informe seu nome e o número do chalé para gerar a comanda Pix.")
                .color(MUTED),
        );
        ui.add_space(10.0);
        egui::Grid::new("checkout_grid")
            .num_columns(2)
            .spacing(Vec2::new(14.0, 10.0))
            .show(ui, |ui| {
                ui.label("Cliente");
                ui.add(
                    egui::TextEdit::singleline(&mut self.customer_name)
                        .hint_text("Seu nome")
                        .desired_width(280.0),
                );
                ui.end_row();
                ui.label("Chalé");
                ui.add(
                    egui::TextEdit::singleline(&mut self.chalet_number)
                        .hint_text("Número do chalé")
                        .desired_width(280.0),
                );
                ui.end_row();
            });
        ui.add_space(14.0);
        let enabled = !self.checkout_busy
            && !self.customer_name.trim().is_empty()
            && !self.chalet_number.trim().is_empty();
        if ui
            .add_enabled_ui(enabled, |ui| {
                self.lucide_text_button(
                    ui,
                    Icon::ShoppingBag,
                    if self.checkout_busy {
                        "Criando comanda"
                    } else {
                        "Gerar comanda Pix"
                    },
                    210.0,
                    44.0,
                    Color32::from_rgb(111, 74, 49),
                )
            })
            .inner
            .clicked()
        {
            self.create_order();
        }
    }

    fn show_pix_checkout(&mut self, ui: &mut egui::Ui) {
        Frame::new().fill(PANEL).stroke(Stroke::new(1.0,BORDER)).corner_radius(14).inner_margin(24.0).show(ui, |ui| {
            ui.heading(RichText::new("Pagamento via Pix").size(22.0).strong().color(TEXT));
            if let Some(number) = &self.order_number { ui.label(RichText::new(format!("Comanda {number}")).color(GOLD).strong()); }
            ui.label(RichText::new("Faça o pagamento e anexe o comprovante para enviar o resumo pelo WhatsApp.").color(MUTED));
            ui.add_space(18.0);
            ui.label(RichText::new("CHAVE PIX · E-MAIL").size(11.0).strong().color(MUTED));
            ui.horizontal(|ui| {
                ui.label(RichText::new("matto.corporativo@gmail.com").size(17.0).strong().color(TEXT));
                if self.lucide_text_button(ui, Icon::Copy, "Copiar chave", 118.0, 34.0, PANEL_HI).clicked() { ui.ctx().copy_text("matto.corporativo@gmail.com".into()); self.notice = Some("Chave Pix copiada.".into()); }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(format!("Valor: {}", currency(self.order_total))).size(18.0).strong().color(GOLD));
            ui.add_space(18.0);
            if let Some(path) = &self.receipt_path {
                ui.label(RichText::new(format!("Comprovante: {}", path.file_name().and_then(|name| name.to_str()).unwrap_or("imagem"))).color(GREEN));
            }
            if let Some(texture) = &self.receipt_preview {
                ui.add_space(8.0);
                ui.add(egui::Image::new((texture.id(),Vec2::new(430.0,320.0))).shrink_to_fit());
            }
            ui.horizontal(|ui| {
                if ui.add_enabled_ui(!self.receipt_busy, |ui| self.lucide_text_button(
                    ui,
                    Icon::Upload,
                    if self.receipt_path.is_some() { "Escolher outro comprovante" } else { "Anexar comprovante" },
                    220.0,
                    38.0,
                    PANEL_HI,
                )).inner.clicked() {
                    self.choose_receipt();
                }
                if self.receipt_path.is_some() && ui.add_enabled_ui(!self.receipt_busy, |ui| self.lucide_text_button(
                    ui,
                    Icon::ArrowRight,
                    if self.receipt_busy { "Enviando" } else { "Enviar e abrir WhatsApp" },
                    230.0,
                    38.0,
                    Color32::from_rgb(111, 74, 49),
                )).inner.clicked() {
                    self.submit_receipt();
                }
            });
        });
    }

    fn choose_receipt(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagem do comprovante", &["jpg", "jpeg", "png", "webp"])
            .pick_file()
        {
            if let Ok(image) = image::open(&path) {
                let image = image.thumbnail(900, 700).to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
                self.receipt_preview = Some(self.last_context.load_texture(
                    "receipt-preview",
                    color,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.receipt_path = Some(path);
        }
    }

    fn submit_receipt(&mut self) {
        let (Some(path), Some(token)) = (self.receipt_path.clone(), self.order_token.clone())
        else {
            return;
        };
        self.receipt_busy = true;
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        thread::spawn(move || {
            let result =
                prepare_jpeg(&path)
                    .and_then(|data_url| {
                        reqwest::blocking::Client::new().post(format!("{}/orders",api::API_BASE))
                    .json(&serde_json::json!({"action":"proof","token":token,"dataUrl":data_url}))
                    .send().and_then(|response| response.error_for_status().map(|_| ()))
                    .map_err(anyhow::Error::from)
                    })
                    .map_err(|error| error.to_string());
            let _ = tx.send(Event::ProofSubmitted(result));
            ctx.request_repaint();
        });
    }

    fn create_order(&mut self) {
        self.checkout_busy = true;
        self.error = None;
        let entries = self.cart.clone();
        let customer = self.customer_name.trim().to_owned();
        let chalet = self.chalet_number.trim().to_owned();
        let repaint_sender = self.tx.clone();
        // The network request is launched on a worker so the native interface stays responsive.
        let ctx = self.last_context.clone();
        thread::spawn(move || {
            let items: Vec<_> = entries.iter().map(|item| serde_json::json!({"productId": item.product_id, "quantity": item.quantity})).collect();
            let result = reqwest::blocking::Client::new()
                .post(format!("{}/orders", api::API_BASE))
                .json(&serde_json::json!({"customerName": customer, "chaletNumber": chalet, "items": items, "notes": ""}))
                .send().and_then(|response| response.error_for_status()?.json::<api::CreatedOrderResponse>())
                .map_err(|error| error.to_string());
            let _ = repaint_sender.send(Event::OrderCreated(result));
            ctx.request_repaint();
        });
    }

    fn authenticate_admin(&mut self) {
        self.admin_busy = true;
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
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

    fn show_settings(&mut self, ui: &mut egui::Ui) {
        self.page_header(ui, "Configurações", "Gerencie o catálogo de produtos.");
        ui.horizontal(|ui| {
            if self
                .lucide_text_button(
                    ui,
                    Icon::Plus,
                    "Novo produto",
                    156.0,
                    38.0,
                    Color32::from_rgb(111, 74, 49),
                )
                .clicked()
            {
                self.reset_product_form();
                self.admin_form_open = true;
            }
            ui.label(RichText::new("As alterações são salvas no catálogo online.").color(MUTED));
        });
        ui.add_space(12.0);
        egui::Grid::new("admin_categories")
            .num_columns(3)
            .spacing(Vec2::new(8.0, 0.0))
            .show(ui, |ui| {
                for (filter, label) in [
                    (CategoryFilter::Todos, "Todos"),
                    (CategoryFilter::Adega, "Adega"),
                    (CategoryFilter::Frigobar, "Frigobar"),
                ] {
                    let active = self.admin_filter == filter;
                    if ui
                        .add(
                            egui::Button::new(label)
                                .fill(if active {
                                    Color32::from_rgb(111, 74, 49)
                                } else {
                                    PANEL
                                })
                                .stroke(Stroke::new(1.0, BORDER))
                                .corner_radius(16),
                        )
                        .clicked()
                    {
                        self.admin_filter = filter;
                    }
                }
            });
        ui.add_space(12.0);
        let form_open = self.admin_form_open;
        if form_open {
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(12)
                .inner_margin(18.0)
                .show(ui, |ui| {
                    ui.heading(
                        RichText::new(if self.editing_id.is_some() {
                            "Editar produto"
                        } else {
                            "Novo produto"
                        })
                        .color(TEXT)
                        .size(18.0),
                    );
                    ui.add_space(12.0);
                    egui::Grid::new("product_form")
                        .num_columns(2)
                        .spacing(Vec2::new(14.0, 10.0))
                        .show(ui, |ui| {
                            text_field(
                                ui,
                                "Nome do produto",
                                &mut self.admin_form.name,
                                "Nome interno",
                            );
                            text_field(
                                ui,
                                "Título do cartão",
                                &mut self.admin_form.label,
                                "Título exibido",
                            );
                            text_field(ui, "Preço normal", &mut self.admin_form.price, "R$ 0,00");
                            text_field(
                                ui,
                                "Preço promocional",
                                &mut self.admin_form.promo_price,
                                "Opcional",
                            );
                            text_field(
                                ui,
                                "Quantidade em estoque",
                                &mut self.admin_form.quantity,
                                "Opcional",
                            );
                            ui.label("Categoria");
                            ui.horizontal(|ui| {
                                for (filter, label) in [
                                    (CategoryFilter::Adega, "Adega"),
                                    (CategoryFilter::Frigobar, "Frigobar"),
                                ] {
                                    if ui
                                        .selectable_label(self.admin_form.category == filter, label)
                                        .clicked()
                                    {
                                        self.admin_form.category = filter;
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label("Tom da garrafa");
                            egui::ComboBox::from_id_salt("product_tone")
                                .selected_text(tone_label(&self.admin_form.tone))
                                .show_ui(ui, |ui| {
                                    for (value, label) in [
                                        ("ivory", "Clara"),
                                        ("gold", "Dourada"),
                                        ("olive", "Oliva"),
                                    ] {
                                        ui.selectable_value(
                                            &mut self.admin_form.tone,
                                            value.to_owned(),
                                            label,
                                        );
                                    }
                                });
                            ui.end_row();
                            ui.label("Descrição");
                            ui.add(
                                egui::TextEdit::multiline(&mut self.admin_form.description)
                                    .desired_rows(3)
                                    .desired_width(430.0),
                            );
                            ui.end_row();
                        });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled_ui(!self.product_image_upload_busy, |ui| {
                                self.lucide_text_button(
                                    ui,
                                    Icon::ImagePlus,
                                    if self.product_image_upload_busy {
                                        "Enviando imagem"
                                    } else {
                                        "Anexar imagem do computador"
                                    },
                                    240.0,
                                    36.0,
                                    PANEL_HI,
                                )
                            })
                            .inner
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
                                    .unwrap_or("Sem imagem"),
                            )
                            .color(MUTED)
                            .size(12.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if self
                            .lucide_text_button(
                                ui,
                                Icon::ChevronLeft,
                                "Cancelar",
                                108.0,
                                36.0,
                                PANEL_HI,
                            )
                            .clicked()
                        {
                            self.reset_product_form();
                        }
                        if ui
                            .add(
                                egui::Button::new("Salvar produto")
                                    .fill(Color32::from_rgb(111, 74, 49)),
                            )
                            .clicked()
                        {
                            self.save_product();
                        }
                    });
                });
            ui.add_space(15.0);
        }
        ui.heading(RichText::new("Catálogo").size(18.0).color(TEXT));
        ui.add_space(10.0);
        let products: Vec<Product> = self
            .products
            .iter()
            .filter(|product| {
                self.admin_filter == CategoryFilter::Todos
                    || (self.admin_filter == CategoryFilter::Adega && product.category == "adega")
                    || (self.admin_filter == CategoryFilter::Frigobar
                        && product.category == "frigobar")
            })
            .cloned()
            .collect();
        for product in products {
            Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(8)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&product.label).strong().color(TEXT));
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}",
                                    product.category.to_uppercase(),
                                    currency(price(&product))
                                ))
                                .color(GOLD),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self
                                .lucide_text_button(
                                    ui,
                                    Icon::Trash,
                                    "Excluir",
                                    88.0,
                                    32.0,
                                    PANEL_HI,
                                )
                                .clicked()
                            {
                                self.delete_product(product.id.clone());
                            }
                            if self
                                .lucide_text_button(
                                    ui,
                                    Icon::Pencil,
                                    "Editar",
                                    84.0,
                                    32.0,
                                    PANEL_HI,
                                )
                                .clicked()
                            {
                                self.fill_product_form(&product);
                            }
                        });
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn reset_product_form(&mut self) {
        self.admin_form = ProductForm::default();
        self.admin_form.category = CategoryFilter::Adega;
        self.admin_form.tone = "ivory".into();
        self.editing_id = None;
        self.admin_form_open = false;
    }

    fn fill_product_form(&mut self, product: &Product) {
        self.admin_form.name = product.name.clone();
        self.admin_form.label = product.label.clone();
        self.admin_form.price = product.price.clone();
        self.admin_form.promo_price = product.promotional_price.clone().unwrap_or_default();
        self.admin_form.quantity = product
            .stock_quantity
            .map(|value| value.to_string())
            .unwrap_or_default();
        self.admin_form.description = product.description.clone();
        self.admin_form.image_url = product.image_url.clone().unwrap_or_default();
        self.admin_form.category = if product.category == "frigobar" {
            CategoryFilter::Frigobar
        } else {
            CategoryFilter::Adega
        };
        self.admin_form.tone = product.tone.clone();
        self.editing_id = Some(product.id.clone());
        self.admin_form_open = true;
    }

    fn save_product(&mut self) {
        let form = &self.admin_form;
        let category = if form.category == CategoryFilter::Frigobar {
            "frigobar"
        } else {
            "adega"
        };
        let quantity = form.quantity.trim().parse::<u32>().ok();
        let payload = serde_json::json!({
            "name": form.name.trim(), "label": form.label.trim(), "price": form.price.trim(),
            "promotionalPrice": form.promo_price.trim(), "description": form.description.trim(),
            "imageUrl": form.image_url.trim(), "category": category, "tone": form.tone,
            "stockQuantity": quantity,
        });
        let id = self.editing_id.clone();
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        thread::spawn(move || {
            let endpoint = id
                .as_ref()
                .map(|id| format!("{}/products?id={}", api::API_BASE, urlencoding::encode(id)))
                .unwrap_or_else(|| format!("{}/products", api::API_BASE));
            let builder = if id.is_some() {
                reqwest::blocking::Client::new().put(endpoint)
            } else {
                reqwest::blocking::Client::new().post(endpoint)
            };
            let result = builder
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
                .and_then(|response| response.error_for_status().map(|_| id.clone()))
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
        let token = self.admin_token.trim().to_owned();
        let tx = self.tx.clone();
        let ctx = self.last_context.clone();
        self.product_image_upload_busy = true;
        thread::spawn(move || {
            let result = prepare_product_image(&path).and_then(|data_url| {
                reqwest::blocking::Client::new().post(format!("{}/product-image",api::API_BASE)).bearer_auth(token)
                    .json(&serde_json::json!({"fileName": path.file_name().and_then(|name| name.to_str()).unwrap_or("produto.png"),"dataUrl":data_url}))
                    .send().and_then(|response| response.error_for_status()?.json::<serde_json::Value>())
                    .map_err(anyhow::Error::from)
                    .and_then(|value| value.get("imageUrl").and_then(|url|url.as_str()).map(str::to_owned).ok_or_else(||anyhow::anyhow!("O servidor não retornou a URL da imagem")))
            }).map_err(|error|error.to_string());
            let _ = tx.send(Event::ProductImageUploaded(result));
            ctx.request_repaint();
        });
    }
}

impl eframe::App for MattoApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.last_context = ctx.clone();
        self.handle_events(&ctx);
        egui::Panel::left("navigation")
            .default_size(226.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin {
                        left: 18,
                        right: 18,
                        top: 18,
                        bottom: 16,
                    })
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    if let Some(brand) = &self.brand_texture {
                        ui.add(
                            egui::Image::new((brand.id(), Vec2::new(46.0, 33.0)))
                                .fit_to_exact_size(Vec2::new(46.0, 33.0)),
                        );
                    }
                    ui.label(RichText::new("Matto ").size(20.0).strong().color(TEXT));
                    ui.label(RichText::new("Chalés").size(20.0).strong().color(GOLD));
                });
                ui.add_space(28.0);
                ui.label(
                    RichText::new("MENU PRINCIPAL")
                        .size(10.0)
                        .strong()
                        .color(MUTED),
                );
                ui.add_space(8.0);
                self.nav_item(ui, Page::Home, Icon::House, "Início");
                self.nav_item(ui, Page::Buscar, Icon::Search, "Buscar produtos");
                self.nav_item(ui, Page::Carrinho, Icon::ShoppingCart, "Carrinho");
                ui.add_space(22.0);
                ui.separator();
                if self.admin_unlocked {
                    self.nav_item(ui, Page::Configuracoes, Icon::Settings, "Configurações");
                } else {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("ÁREA ADMINISTRATIVA")
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.add_space(8.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.admin_token)
                            .password(true)
                            .hint_text("Token administrativo")
                            .desired_width(188.0),
                    );
                    ui.add_space(6.0);
                    if self
                        .lucide_text_button(
                            ui,
                            Icon::Settings,
                            "Entrar nas configurações",
                            188.0,
                            36.0,
                            PANEL_HI,
                        )
                        .clicked()
                    {
                        self.authenticate_admin();
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.separator();
                    ui.label(
                        RichText::new("Vinhos escolhidos\npara bons momentos.")
                            .color(MUTED)
                            .size(12.0),
                    );
                });
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::same(28)))
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let title = match self.page {
                            Page::Home => "Bem-vindo à adega",
                            Page::Buscar => "Catálogo",
                            Page::Carrinho => "Seu pedido",
                            Page::Configuracoes => "Configurações",
                            Page::Detalhe => "Produto",
                        };
                        ui.label(RichText::new(title).size(14.0).color(MUTED));
                        ui.label(
                            RichText::new("MATTO CHALÉS")
                                .size(11.0)
                                .strong()
                                .color(GOLD),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self
                            .lucide_text_button(
                                ui,
                                Icon::ShoppingCart,
                                &format!("Carrinho ({})", self.cart_count()),
                                150.0,
                                38.0,
                                PANEL,
                            )
                            .clicked()
                        {
                            self.page = Page::Carrinho;
                        }
                    });
                });
                ui.add_space(20.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        match self.page {
                            Page::Home => self.show_home(ui, &ctx),
                            Page::Buscar => self.show_search(ui),
                            Page::Carrinho => self.show_cart(ui),
                            Page::Detalhe => self.show_details(ui),
                            Page::Configuracoes => self.show_settings(ui),
                        }
                        if let Some(message) = &self.notice {
                            ui.add_space(14.0);
                            ui.label(RichText::new(message).color(GREEN));
                        }
                        if let Some(message) = &self.error {
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(message).color(Color32::from_rgb(226, 113, 102)),
                            );
                        }
                    });
            });
    }
}

fn set_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = PANEL;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = Color32::from_rgb(105, 61, 32);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = PANEL_HI;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(76, 47, 27);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.bg_fill = Color32::from_rgb(138, 79, 36);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    ctx.set_visuals(visuals);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = Vec2::new(12.0, 12.0);
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn load_state() -> (Vec<CartEntry>, Option<PathBuf>) {
    let path = directories::ProjectDirs::from("br", "Matto Chales", "Matto Chales")
        .map(|dirs| dirs.data_local_dir().join("state.json"));
    let cart = path
        .as_ref()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<PersistedState>(&bytes).ok())
        .map(|state| state.cart)
        .unwrap_or_default();
    (cart, path)
}

fn price(product: &Product) -> f64 {
    let value = product
        .promotional_price
        .as_deref()
        .unwrap_or(&product.price);
    let cleaned: String = value
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == ',' || *ch == '.')
        .collect();
    let normalized = if cleaned.contains(',') {
        cleaned.replace('.', "").replace(',', ".")
    } else {
        cleaned
    };
    normalized.parse().unwrap_or(0.0)
}

fn currency(value: f64) -> String {
    format!("R$ {value:.2}").replace('.', ",")
}
fn tone_label(tone: &str) -> &'static str {
    match tone {
        "gold" => "Dourada",
        "olive" => "Oliva",
        _ => "Clara",
    }
}

fn prepare_jpeg(path: &PathBuf) -> anyhow::Result<String> {
    let mut image = image::open(path)?.to_rgb8();
    let (width, height) = image.dimensions();
    if width.max(height) > 1800 {
        image = image::imageops::resize(
            &image,
            width * 1800 / width.max(height),
            height * 1800 / width.max(height),
            image::imageops::FilterType::Lanczos3,
        );
    }
    for quality in [82, 72, 62, 52] {
        let mut bytes = Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
            .encode_image(&image)?;
        if bytes.get_ref().len() <= 1_700_000 {
            return Ok(format!(
                "data:image/jpeg;base64,{}",
                BASE64.encode(bytes.into_inner())
            ));
        }
    }
    anyhow::bail!("O comprovante ficou muito grande. Escolha uma imagem menor.")
}

fn prepare_product_image(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = fs::read(path)?;
    let extension = path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or("jpg")
        .to_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        _ => anyhow::bail!("Formato de imagem não suportado"),
    };
    if bytes.len() > 1_700_000 {
        anyhow::bail!("Imagem acima do limite de 1,7 MB.");
    }
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(label);
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(360.0),
    );
    ui.end_row();
}
