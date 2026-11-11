use eframe::egui;
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, PartialEq)]
pub struct ImageDef {
    pub id: String,
    pub name: String,
    pub pos: (i32, i32),
    pub size: (u32, u32),
    pub flags: String, // e.g. "0", "ISVerticalTile", "ISHorizontalTile", "3"
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageSet {
    pub name: String,
    pub ref_size: (u32, u32),
    pub texture_path_1x: String, // Standard mpix 0 texture path
    pub texture_path_2x: String, // Optional mpix 1 (@2x) texture path
    pub enable_2x: bool,
    pub images: Vec<ImageDef>,
}

impl Default for ImageSet {
    fn default() -> Self {
        Self {
            name: "dayz_gui".to_string(),
            ref_size: (1024, 1024),
            texture_path_1x: "Gui/imagesets/dayz_gui.edds".to_string(),
            texture_path_2x: "Gui/imagesets/dayz_gui@2x.edds".to_string(),
            enable_2x: false,
            images: Vec::new(),
        }
    }
}

pub fn sanitize_ident(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "icon_item".to_string()
    } else {
        cleaned
    }
}

pub fn parse_imageset(content: &str) -> ImageSet {
    let mut imageset = ImageSet::default();

    let re_name = Regex::new(r#"ImageSetClass\s*\{[\s\S]*?Name\s+["\\]([^"\\\r\n]+)["\\]"#).unwrap();
    if let Some(caps) = re_name.captures(content) {
        imageset.name = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();
    }

    let re_ref = Regex::new(r#"RefSize\s+(\d+)\s+(\d+)"#).unwrap();
    if let Some(caps) = re_ref.captures(content) {
        let w = caps[1].parse::<u32>().unwrap_or(1024);
        let h = caps[2].parse::<u32>().unwrap_or(1024);
        imageset.ref_size = (w, h);
    }

    // Parse Textures: mpix 0 and mpix 1 (รองรับ GUID prefix เช่น {534691EE0479871C}...)
    let re_texture_entry = Regex::new(r#"ImageSetTextureClass\s*\{[\s\S]*?mpix\s+(\d+)[\s\S]*?path\s+["\\]([^"\\\r\n]+)["\\]"#).unwrap();
    let mut found_textures = Vec::new();
    for caps in re_texture_entry.captures_iter(content) {
        let mpix = caps[1].parse::<u32>().unwrap_or(0);
        let path = caps.get(2).map_or("", |m| m.as_str()).trim().to_string();
        found_textures.push((mpix, path));
    }

    if !found_textures.is_empty() {
        // Find mpix 0 or first texture
        if let Some((_, p)) = found_textures.iter().find(|(m, _)| *m == 0) {
            imageset.texture_path_1x = p.clone();
        } else {
            imageset.texture_path_1x = found_textures[0].1.clone();
        }

        // Find mpix 1 (@2x)
        if let Some((_, p)) = found_textures.iter().find(|(m, _)| *m == 1) {
            imageset.texture_path_2x = p.clone();
            imageset.enable_2x = true;
        } else {
            imageset.enable_2x = false;
        }
    } else {
        // Fallback simple path
        let re_path = Regex::new(r#"path\s+["\\]([^"\\\r\n]+)["\\]"#).unwrap();
        if let Some(caps) = re_path.captures(content) {
            imageset.texture_path_1x = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();
        }
    }

    let mut images = Vec::new();
    let re_group = Regex::new(r#"ImageSetGroupClass\s+([^\s{]+)\s*\{[\s\S]*?Name\s+["\\]([^"\\\r\n]+)["\\][\s\S]*?Images\s*\{([\s\S]*?)\}\s*\}"#).unwrap();
    let re_def = Regex::new(r#"ImageSetDefClass\s+([^\s{]+)\s*\{([\s\S]*?)\}"#).unwrap();
    let re_def_name = Regex::new(r#"Name\s+["\\]([^"\\\r\n]+)["\\]"#).unwrap();
    let re_def_pos = Regex::new(r#"Pos\s+(-?\d+)\s+(-?\d+)"#).unwrap();
    let re_def_size = Regex::new(r#"Size\s+(\d+)\s+(\d+)"#).unwrap();
    let re_def_flags = Regex::new(r#"Flags\s+([^\r\n]+)"#).unwrap();

    let mut group_spans = Vec::new();

    for caps in re_group.captures_iter(content) {
        let full_match = caps.get(0).unwrap();
        group_spans.push((full_match.start(), full_match.end()));

        let group_name = caps.get(2).map_or("", |m| m.as_str()).trim().to_string();
        let group_body = caps.get(3).map_or("", |m| m.as_str());

        for def_caps in re_def.captures_iter(group_body) {
            let class_id = def_caps.get(1).map_or("", |m| m.as_str()).to_string();
            let body = def_caps.get(2).map_or("", |m| m.as_str());

            let name = if let Some(nc) = re_def_name.captures(body) {
                nc.get(1).map_or(class_id.clone(), |m| m.as_str().trim().to_string())
            } else {
                class_id.clone()
            };

            let pos = if let Some(pc) = re_def_pos.captures(body) {
                (pc[1].parse::<i32>().unwrap_or(0), pc[2].parse::<i32>().unwrap_or(0))
            } else {
                (0, 0)
            };

            let size = if let Some(sc) = re_def_size.captures(body) {
                (sc[1].parse::<u32>().unwrap_or(64), sc[2].parse::<u32>().unwrap_or(64))
            } else {
                (64, 64)
            };

            let flags = if let Some(fc) = re_def_flags.captures(body) {
                fc.get(1).map_or("0", |m| m.as_str()).trim().to_string()
            } else {
                "0".to_string()
            };

            images.push(ImageDef {
                id: class_id,
                name,
                pos,
                size,
                flags,
                group: Some(group_name.clone()),
            });
        }
    }

    // Now parse ungrouped definitions
    let mut stripped = content.to_string();
    group_spans.sort_by(|a, b| b.0.cmp(&a.0));
    for (start, end) in group_spans {
        if start < stripped.len() && end <= stripped.len() {
            stripped.replace_range(start..end, " ");
        }
    }

    // ดึง ImageSetDefClass ที่ไม่ได้อยู่ใน Group ทั้งหมดโดยตรงจากเนื้อหาที่ตัด Group ออกแล้ว
    for def_caps in re_def.captures_iter(&stripped) {
        let class_id = def_caps.get(1).map_or("", |m| m.as_str()).to_string();
        let body = def_caps.get(2).map_or("", |m| m.as_str());

        let name = if let Some(nc) = re_def_name.captures(body) {
            nc.get(1).map_or(class_id.clone(), |m| m.as_str().trim().to_string())
        } else {
            class_id.clone()
        };

        let pos = if let Some(pc) = re_def_pos.captures(body) {
            (pc[1].parse::<i32>().unwrap_or(0), pc[2].parse::<i32>().unwrap_or(0))
        } else {
            (0, 0)
        };

        let size = if let Some(sc) = re_def_size.captures(body) {
            (sc[1].parse::<u32>().unwrap_or(64), sc[2].parse::<u32>().unwrap_or(64))
        } else {
            (64, 64)
        };

        let flags = if let Some(fc) = re_def_flags.captures(body) {
            fc.get(1).map_or("0", |m| m.as_str()).trim().to_string()
        } else {
            "0".to_string()
        };

        images.push(ImageDef {
            id: class_id,
            name,
            pos,
            size,
            flags,
            group: None,
        });
    }

    imageset.images = images;
    imageset
}

pub fn generate_imageset(imageset: &ImageSet) -> String {
    let mut out = String::new();

    // Standard DayZ Template Formatting
    out.push_str("ImageSetClass {\n");
    out.push_str(&format!(" Name \"{}\"\n", imageset.name));
    out.push_str(&format!(" RefSize {} {}\n", imageset.ref_size.0, imageset.ref_size.1));
    out.push_str(" Textures {\n");
    
    // mpix 0
    out.push_str("  ImageSetTextureClass {\n");
    out.push_str("   mpix 0\n");
    out.push_str(&format!("   path \"{}\"\n", imageset.texture_path_1x));
    out.push_str("  }\n");

    // mpix 1 (@2x) if enabled
    if imageset.enable_2x {
        out.push_str("  ImageSetTextureClass {\n");
        out.push_str("   mpix 1\n");
        out.push_str(&format!("   path \"{}\"\n", imageset.texture_path_2x));
        out.push_str("  }\n");
    }

    out.push_str(" }\n");

    // Images block
    out.push_str(" Images {\n");
    for img in &imageset.images {
        if img.group.is_none() {
            let clean_id = sanitize_ident(&img.name);
            out.push_str(&format!("  ImageSetDefClass {} {{\n", clean_id));
            out.push_str(&format!("   Name \"{}\"\n", img.name));
            out.push_str(&format!("   Pos {} {}\n", img.pos.0, img.pos.1));
            out.push_str(&format!("   Size {} {}\n", img.size.0, img.size.1));
            let fl = if img.flags.trim().is_empty() { "0" } else { img.flags.trim() };
            out.push_str(&format!("   Flags {}\n", fl));
            out.push_str("  }\n");
        }
    }
    out.push_str(" }\n");

    // Groups block (ใช้ BTreeMap เพื่อให้ลำดับของ Group คงที่เสมอทุกครั้งที่บันทึก)
    let mut groups: BTreeMap<String, Vec<&ImageDef>> = BTreeMap::new();
    for img in &imageset.images {
        if let Some(ref g) = img.group {
            groups.entry(g.clone()).or_default().push(img);
        }
    }

    if !groups.is_empty() {
        out.push_str(" Groups {\n");
        for (group_name, group_images) in &groups {
            let clean_group = sanitize_ident(group_name);
            out.push_str(&format!("  ImageSetGroupClass {} {{\n", clean_group));
            out.push_str(&format!("   Name \"{}\"\n", group_name));
            out.push_str("   Images {\n");
            for img in group_images {
                let clean_id = sanitize_ident(&img.name);
                out.push_str(&format!("    ImageSetDefClass {} {{\n", clean_id));
                out.push_str(&format!("     Name \"{}\"\n", img.name));
                out.push_str(&format!("     Pos {} {}\n", img.pos.0, img.pos.1));
                out.push_str(&format!("     Size {} {}\n", img.size.0, img.size.1));
                let fl = if img.flags.trim().is_empty() { "0" } else { img.flags.trim() };
                out.push_str(&format!("     Flags {}\n", fl));
                out.push_str("    }\n");
            }
            out.push_str("   }\n");
            out.push_str("  }\n");
        }
        out.push_str(" }\n");
    }

    out.push_str("}\n");
    out
}

// ======================== Desktop GUI Application ========================

#[derive(PartialEq, Clone, Copy)]
enum ToolMode {
    DrawEdit,
    Pan,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

struct ImagesetEditorApp {
    imageset: ImageSet,
    selected_index: Option<usize>,
    selected_indices: HashSet<usize>, // รองรับการเลือกหลายกล่องพร้อมกัน (Multi-Selection)
    clipboard_sprites: Vec<ImageDef>, // คลิปบอร์ดสำหรับ Copy / Paste
    undo_stack: Vec<ImageSet>,        // ประวัติการแก้ไขสำหรับ Undo (Ctrl+Z)
    redo_stack: Vec<ImageSet>,        // ประวัติการแก้ไขสำหรับ Redo (Ctrl+Y)
    search_query: String,             // คำค้นหากรองรายชื่อ Sprite
    focus_target_index: Option<usize>,// เป้าหมายการเลื่อน Canvas ไปหากล่องที่เลือก
    status_msg: String,

    // Brand icon texture for top navbar
    brand_icon: Option<egui::TextureHandle>,
    
    // Texture preview
    texture_handle: Option<egui::TextureHandle>,
    loaded_texture_path: Option<PathBuf>,
    loaded_rgba_image: Option<image::RgbaImage>,
    
    // Canvas pan & zoom
    pan: egui::Vec2,
    zoom: f32,
    tool_mode: ToolMode,
    
    // Interactive Box Drawing / Moving / Resizing
    is_drawing: bool,
    draw_start: Option<egui::Pos2>,
    drag_moving_index: Option<usize>,
    drag_moving_offset: egui::Vec2,
    drag_moving_orig_positions: Vec<(usize, (i32, i32))>, // เก็บพิกัดเริ่มต้นของทุกกล่องที่ลากพร้อมกัน
    drag_resizing_index: Option<usize>,
    drag_resize_corner: Option<ResizeCorner>,
    drag_orig_box: Option<(i32, i32, u32, u32)>, // pos.x, pos.y, size.w, size.h
    
    snap_grid: bool,
    grid_size: i32,
    show_labels: bool, // เช็คบ็อกซ์เปิด/ปิดการแสดงชื่อไอคอนบน Canvas
    
    // Live code text
    code_cache: String,
}

impl Default for ImagesetEditorApp {
    fn default() -> Self {
        let imageset = ImageSet::default();
        let code = generate_imageset(&imageset);
        Self {
            imageset,
            selected_index: None,
            selected_indices: HashSet::new(),
            clipboard_sprites: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_query: String::new(),
            focus_target_index: None,
            status_msg: "Ready: Click & drag on canvas to draw sprite boxes, or drop a file into window".to_string(),
            brand_icon: None,
            texture_handle: None,
            loaded_texture_path: None,
            loaded_rgba_image: None,
            pan: egui::vec2(60.0, 60.0),
            zoom: 0.65,
            tool_mode: ToolMode::DrawEdit,
            is_drawing: false,
            draw_start: None,
            drag_moving_index: None,
            drag_moving_offset: egui::Vec2::ZERO,
            drag_moving_orig_positions: Vec::new(),
            drag_resizing_index: None,
            drag_resize_corner: None,
            drag_orig_box: None,
            snap_grid: true,
            grid_size: 8,
            show_labels: true,
            code_cache: code,
        }
    }
}

impl ImagesetEditorApp {
    fn save_undo_state(&mut self) {
        if self.undo_stack.last() != Some(&self.imageset) {
            if self.undo_stack.len() >= 50 {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.imageset.clone());
            self.redo_stack.clear();
        }
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.imageset.clone());
            self.imageset = prev;
            if let Some(idx) = self.selected_index {
                if idx >= self.imageset.images.len() {
                    self.selected_index = self.imageset.images.len().checked_sub(1);
                }
            }
            self.selected_indices.retain(|&i| i < self.imageset.images.len());
            self.status_msg = "Undo applied (Ctrl+Z)".to_string();
            self.update_code();
        } else {
            self.status_msg = "Nothing to undo".to_string();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.imageset.clone());
            self.imageset = next;
            if let Some(idx) = self.selected_index {
                if idx >= self.imageset.images.len() {
                    self.selected_index = self.imageset.images.len().checked_sub(1);
                }
            }
            self.selected_indices.retain(|&i| i < self.imageset.images.len());
            self.status_msg = "Redo applied (Ctrl+Y)".to_string();
            self.update_code();
        } else {
            self.status_msg = "Nothing to redo".to_string();
        }
    }

    fn new_project(&mut self) {
        self.save_undo_state();
        self.imageset = ImageSet {
            name: "MyMod_Icons".to_string(),
            ref_size: (1024, 1024),
            texture_path_1x: "Gui/imagesets/MyMod_Icons.edds".to_string(),
            texture_path_2x: "Gui/imagesets/MyMod_Icons@2x.edds".to_string(),
            enable_2x: false,
            images: Vec::new(),
        };
        self.selected_index = None;
        self.selected_indices.clear();
        self.search_query.clear();
        self.texture_handle = None;
        self.loaded_texture_path = None;
        self.loaded_rgba_image = None;
        self.is_drawing = false;
        self.draw_start = None;
        self.drag_moving_index = None;
        self.drag_moving_orig_positions.clear();
        self.drag_resizing_index = None;
        self.pan = egui::vec2(60.0, 60.0);
        self.zoom = 0.65;
        self.status_msg = "New DayZ .imageset project initialized (All fields reset)".to_string();
        self.update_code();
    }

    fn update_code(&mut self) {
        self.code_cache = generate_imageset(&self.imageset);
    }

    fn snap(&self, val: f32) -> f32 {
        if self.snap_grid && self.grid_size > 0 {
            let gs = self.grid_size as f32;
            (val / gs).round() * gs
        } else {
            val.round()
        }
    }

    // ดึงพาธเครื่องมือ edds2png (มี Embedded Binary ในตัว)
    fn get_edds2png_tool() -> Option<PathBuf> {
        let temp_dir = env::temp_dir().join("dayz_imageset_editor_tools");
        let _ = fs::create_dir_all(&temp_dir);
        let embedded_path = temp_dir.join("edds2png.exe");

        // ตรวจสอบและคลาย binary ออกมาใน Temp หากยังไม่มีหรือขนาดไม่ตรง
        let embedded_bytes = include_bytes!("../edds2png.exe");
        let need_write = if embedded_path.exists() {
            fs::metadata(&embedded_path).map(|m| m.len() != embedded_bytes.len() as u64).unwrap_or(true)
        } else {
            true
        };

        if need_write {
            let _ = fs::write(&embedded_path, embedded_bytes);
        }

        if embedded_path.exists() {
            return Some(embedded_path);
        }

        // Fallback พาธทั่วไป
        let candidates = [
            PathBuf::from("edds2png.exe"),
            PathBuf::from("tools/edds2png.exe"),
            PathBuf::from("P:/workbrench dayz tools/edds2png.exe"),
        ];
        for c in &candidates {
            if c.exists() { return Some(c.clone()); }
        }
        None
    }

    // ดึงพาธเครื่องมือ imagesetconv (มี Embedded Binary ในตัว)
    fn get_imagesetconv_tool() -> Option<PathBuf> {
        let temp_dir = env::temp_dir().join("dayz_imageset_editor_tools");
        let _ = fs::create_dir_all(&temp_dir);
        let embedded_path = temp_dir.join("imagesetconv.exe");

        let embedded_bytes = include_bytes!("../imagesetconv.exe");
        let need_write = if embedded_path.exists() {
            fs::metadata(&embedded_path).map(|m| m.len() != embedded_bytes.len() as u64).unwrap_or(true)
        } else {
            true
        };

        if need_write {
            let _ = fs::write(&embedded_path, embedded_bytes);
        }

        if embedded_path.exists() {
            return Some(embedded_path);
        }

        // Fallback พาธทั่วไป
        let candidates = [
            PathBuf::from("imagesetconv.exe"),
            PathBuf::from("tools/imagesetconv.exe"),
            PathBuf::from("P:/workbrench dayz tools/imagesetconv.exe"),
        ];
        for c in &candidates {
            if c.exists() { return Some(c.clone()); }
        }
        None
    }

    // ฟังก์ชันค้นหาและแปลง .edds ไปเป็น .png โดยใช้ edds2png
    fn convert_edds_to_png(&self, edds_path: &Path) -> Option<PathBuf> {
        let png_path = edds_path.with_extension("png");
        
        // ถ้ามีไฟล์ .png อยู่แล้วและใหม่กว่าไฟล์ .edds สามารถใช้ได้ทันที
        if png_path.exists() {
            if let (Ok(m_edds), Ok(m_png)) = (fs::metadata(edds_path), fs::metadata(&png_path)) {
                if let (Ok(t_edds), Ok(t_png)) = (m_edds.modified(), m_png.modified()) {
                    if t_png >= t_edds {
                        return Some(png_path);
                    }
                }
            }
        }

        if let Some(tool) = Self::get_edds2png_tool() {
            let mut cmd = Command::new(&tool);
            cmd.arg(edds_path);
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.output();

            if png_path.exists() {
                return Some(png_path);
            }
        }

        None
    }

    // เมธอดคัดลอก (Duplicate) กล่อง Sprite ที่เลือก
    fn duplicate_selected_box(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() {
                self.save_undo_state();
                let orig = self.imageset.images[idx].clone();
                let offset = if self.snap_grid && self.grid_size > 0 { self.grid_size } else { 16 };

                let new_sp = ImageDef {
                    id: format!("{}_copy", orig.id),
                    name: format!("{}_copy", orig.name),
                    pos: (orig.pos.0 + offset, orig.pos.1 + offset),
                    size: orig.size,
                    flags: orig.flags,
                    group: orig.group,
                };

                self.imageset.images.push(new_sp);
                let new_idx = self.imageset.images.len() - 1;
                self.selected_index = Some(new_idx);
                self.selected_indices.clear();
                self.selected_indices.insert(new_idx);
                self.status_msg = format!("Duplicated sprite: {}", self.imageset.images.last().unwrap().name);
                self.update_code();
            }
        }
    }

    // คัดลอกกล่องที่เลือกเข้าสู่ Clipboard (Ctrl+C)
    fn copy_selected_sprites(&mut self) {
        let mut indices: Vec<usize> = if !self.selected_indices.is_empty() {
            self.selected_indices.iter().copied().filter(|&i| i < self.imageset.images.len()).collect()
        } else if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() { vec![idx] } else { Vec::new() }
        } else {
            Vec::new()
        };

        if indices.is_empty() {
            return;
        }
        indices.sort_unstable();
        self.clipboard_sprites = indices.into_iter().map(|i| self.imageset.images[i].clone()).collect();
        self.status_msg = format!("Copied {} sprite(s) to clipboard (Ctrl+C)", self.clipboard_sprites.len());
    }

    // วางกล่องจาก Clipboard (Ctrl+V)
    fn paste_sprites(&mut self) {
        if self.clipboard_sprites.is_empty() {
            return;
        }
        self.save_undo_state();
        let offset = if self.snap_grid && self.grid_size > 0 { self.grid_size } else { 16 };
        self.selected_indices.clear();

        let to_paste = self.clipboard_sprites.clone();
        for sp in to_paste {
            let new_name = format!("{}_paste", sp.name);
            let new_sp = ImageDef {
                id: sanitize_ident(&new_name),
                name: new_name,
                pos: (sp.pos.0 + offset, sp.pos.1 + offset),
                size: sp.size,
                flags: sp.flags,
                group: sp.group,
            };
            self.imageset.images.push(new_sp);
            let new_idx = self.imageset.images.len() - 1;
            self.selected_indices.insert(new_idx);
            self.selected_index = Some(new_idx);
        }
        self.status_msg = format!("Pasted {} sprite(s) (Ctrl+V)", self.selected_indices.len());
        self.update_code();
    }

    // ขยับกล่องด้วยปุ่มลูกศรคีย์บอร์ด (Arrow Keys Nudge)
    fn nudge_selected(&mut self, dx: i32, dy: i32) {
        let indices: Vec<usize> = if !self.selected_indices.is_empty() {
            self.selected_indices.iter().copied().filter(|&i| i < self.imageset.images.len()).collect()
        } else if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() { vec![idx] } else { Vec::new() }
        } else {
            Vec::new()
        };

        if indices.is_empty() {
            return;
        }

        self.save_undo_state();
        for idx in indices {
            let sp = &mut self.imageset.images[idx];
            sp.pos.0 = (sp.pos.0 + dx).max(0);
            sp.pos.1 = (sp.pos.1 + dy).max(0);
        }
        self.update_code();
    }

    // 🪄 Auto-Trim Box: สแกนพิกเซลที่ไม่โปร่งใส (Alpha > 10) เพื่อหดกรอบให้พอดีขอบไอคอนอัตโนมัติ
    fn auto_trim_selected_box(&mut self) {
        let img = match self.loaded_rgba_image {
            Some(ref img) => img.clone(),
            None => {
                self.status_msg = "Please load a texture image first before using Auto-Trim!".to_string();
                return;
            }
        };

        let indices: Vec<usize> = if !self.selected_indices.is_empty() {
            self.selected_indices.iter().copied().filter(|&i| i < self.imageset.images.len()).collect()
        } else if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() { vec![idx] } else { Vec::new() }
        } else {
            self.status_msg = "Select a sprite box first to Auto-Trim!".to_string();
            return;
        };

        self.save_undo_state();
        let img_w = img.width() as i32;
        let img_h = img.height() as i32;
        let mut trimmed_count = 0;

        for idx in indices {
            let sp = &mut self.imageset.images[idx];
            let x0 = sp.pos.0.clamp(0, img_w.saturating_sub(1));
            let y0 = sp.pos.1.clamp(0, img_h.saturating_sub(1));
            let x1 = (sp.pos.0 + sp.size.0 as i32).clamp(0, img_w);
            let y1 = (sp.pos.1 + sp.size.1 as i32).clamp(0, img_h);

            let mut min_x = x1;
            let mut min_y = y1;
            let mut max_x = x0;
            let mut max_y = y0;
            let mut found_alpha = false;

            for y in y0..y1 {
                for x in x0..x1 {
                    let pixel = img.get_pixel(x as u32, y as u32);
                    if pixel[3] > 10 {
                        if x < min_x { min_x = x; }
                        if y < min_y { min_y = y; }
                        if x > max_x { max_x = x; }
                        if y > max_y { max_y = y; }
                        found_alpha = true;
                    }
                }
            }

            if found_alpha && max_x >= min_x && max_y >= min_y {
                sp.pos = (min_x, min_y);
                sp.size = ((max_x - min_x + 1).max(4) as u32, (max_y - min_y + 1).max(4) as u32);
                trimmed_count += 1;
            }
        }

        if trimmed_count > 0 {
            self.status_msg = format!("🪄 Auto-Trimmed {} sprite box(es) to tight pixel bounds!", trimmed_count);
            self.update_code();
        } else {
            self.status_msg = "No non-transparent pixels (Alpha > 10) found inside selected box.".to_string();
        }
    }

    // 🔍 Auto-Detect All Icons: สแกนหาเกาะพิกเซลที่ไม่โปร่งใสทั้งแผ่นภาพเพื่อสร้างกรอบ Sprite อัตโนมัติ
    fn auto_detect_all_icons(&mut self) {
        let img = match self.loaded_rgba_image {
            Some(ref img) => img.clone(),
            None => {
                self.status_msg = "Please load a texture image first before running Auto-Detect!".to_string();
                return;
            }
        };

        let w = img.width() as usize;
        let h = img.height() as usize;
        if w == 0 || h == 0 {
            return;
        }

        let mut visited = vec![false; w * h];
        let mut raw_boxes: Vec<(i32, i32, i32, i32)> = Vec::new(); // (min_x, min_y, max_x, max_y)

        // BFS Connected-Components (ขยายรัศมีเชื่อมจุด 2px เพื่อให้ไอคอนที่มีชิ้นส่วนแยกกันเล็กน้อยรวมเป็นกล่องเดียวกัน)
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if !visited[idx] && img.get_pixel(x as u32, y as u32)[3] > 15 {
                    let mut min_x = x as i32;
                    let mut min_y = y as i32;
                    let mut max_x = x as i32;
                    let mut max_y = y as i32;

                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back((x, y));
                    visited[idx] = true;

                    while let Some((cx, cy)) = queue.pop_front() {
                        let cx_i = cx as i32;
                        let cy_i = cy as i32;
                        if cx_i < min_x { min_x = cx_i; }
                        if cy_i < min_y { min_y = cy_i; }
                        if cx_i > max_x { max_x = cx_i; }
                        if cy_i > max_y { max_y = cy_i; }

                        let y_start = cy.saturating_sub(2);
                        let y_end = (cy + 2).min(h - 1);
                        let x_start = cx.saturating_sub(2);
                        let x_end = (cx + 2).min(w - 1);

                        for ny in y_start..=y_end {
                            for nx in x_start..=x_end {
                                let n_idx = ny * w + nx;
                                if !visited[n_idx] && img.get_pixel(nx as u32, ny as u32)[3] > 15 {
                                    visited[n_idx] = true;
                                    queue.push_back((nx, ny));
                                }
                            }
                        }
                    }

                    let bw = max_x - min_x + 1;
                    let bh = max_y - min_y + 1;
                    // กรองฝุ่นพิกเซลขนาดเล็กกว่า 6x6 ออก
                    if bw >= 6 && bh >= 6 {
                        raw_boxes.push((min_x, min_y, max_x, max_y));
                    }
                }
            }
        }

        if raw_boxes.is_empty() {
            self.status_msg = "No distinct icon islands detected (image may be fully opaque or empty).".to_string();
            return;
        }

        // เรียงลำดับจากบนลงล่าง (เป็นแถวๆ) และซ้ายไปขวา
        raw_boxes.sort_by(|a, b| {
            let row_a = a.1 / 16;
            let row_b = b.1 / 16;
            if row_a == row_b {
                a.0.cmp(&b.0)
            } else {
                row_a.cmp(&row_b)
            }
        });

        self.save_undo_state();
        let mut added = 0;
        for (min_x, min_y, max_x, max_y) in raw_boxes {
            let bw = (max_x - min_x + 1) as u32;
            let bh = (max_y - min_y + 1) as u32;

            // ข้ามกล่องที่มีอยู่แล้วในตำแหน่งใกล้เคียงกัน
            let already_exists = self.imageset.images.iter().any(|sp| {
                (sp.pos.0 - min_x).abs() <= 4 && (sp.pos.1 - min_y).abs() <= 4
            });

            if !already_exists {
                let next_num = self.imageset.images.len() + 1;
                self.imageset.images.push(ImageDef {
                    id: format!("icon_{}", next_num),
                    name: format!("icon_{}", next_num),
                    pos: (min_x, min_y),
                    size: (bw, bh),
                    flags: "0".to_string(),
                    group: None,
                });
                added += 1;
            }
        }

        if added > 0 {
            self.selected_index = Some(self.imageset.images.len() - 1);
            self.status_msg = format!("🔍 Auto-Detected & created {} new icon sprite(s)!", added);
            self.update_code();
        } else {
            self.status_msg = "All detected icons already have bounding boxes.".to_string();
        }
    }

    // เมธอดลบ (Delete) กล่อง Sprite ที่เลือก (รองรับทั้งกล่องเดี่ยวและ Multi-selection)
    fn delete_selected_box(&mut self) {
        if !self.selected_indices.is_empty() {
            self.save_undo_state();
            let mut indices: Vec<usize> = self.selected_indices.iter().copied().collect();
            indices.sort_unstable_by(|a, b| b.cmp(a)); // ลบจากท้ายสุดมาหน้าสุดเพื่อไม่ให้ index เคลื่อน

            for idx in indices {
                if idx < self.imageset.images.len() {
                    self.imageset.images.remove(idx);
                }
            }
            self.selected_indices.clear();
            self.selected_index = if !self.imageset.images.is_empty() { Some(self.imageset.images.len() - 1) } else { None };
            self.status_msg = "Deleted selected sprites".to_string();
            self.update_code();
        } else if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() {
                self.save_undo_state();
                let name = self.imageset.images[idx].name.clone();
                self.imageset.images.remove(idx);
                self.selected_index = if !self.imageset.images.is_empty() {
                    Some(idx.min(self.imageset.images.len() - 1))
                } else {
                    None
                };
                self.status_msg = format!("Deleted sprite: {}", name);
                self.update_code();
            }
        }
    }

    // เมธอดเลือกกล่องทั้งหมด (Select All - Ctrl+A)
    fn select_all_sprites(&mut self) {
        self.selected_indices.clear();
        for i in 0..self.imageset.images.len() {
            self.selected_indices.insert(i);
        }
        if !self.imageset.images.is_empty() {
            self.selected_index = Some(0);
        }
        self.status_msg = format!("Selected all {} sprites", self.imageset.images.len());
    }

    // ฟังก์ชันค้นหาไฟล์ .meta ของ .edds เพื่อดึง GUID และ Resource Name
    fn find_meta_guid_and_path(&self, edds_path: &Path) -> Option<(String, String)> {
        let meta_path = PathBuf::from(format!("{}.meta", edds_path.display()));
        if meta_path.exists() {
            if let Ok(content) = fs::read_to_string(&meta_path) {
                // Name "{534691EE0479871C}Gui/imagesets/dayz_gui.tga"
                let re_meta_name = Regex::new(r#"Name\s+["\\]?\{([A-Fa-f0-9]+)\}([^"\\\r\n]+)"#).unwrap();
                if let Some(caps) = re_meta_name.captures(&content) {
                    let guid = caps.get(1).map_or("", |m| m.as_str()).to_string();
                    let raw_path = caps.get(2).map_or("", |m| m.as_str()).trim().to_string();
                    // เปลี่ยน extension จาก .tga หรือ .png ให้เป็น .edds
                    let edds_virtual_path = Path::new(&raw_path).with_extension("edds").to_string_lossy().replace('\\', "/");
                    return Some((guid, edds_virtual_path));
                }
            }
        }
        None
    }

    // ฟังก์ชันสร้างหรือดึง GUID สำหรับโปรเจกต์
    fn get_or_generate_guid(&mut self) -> String {
        let re_guid = Regex::new(r#"^\{([A-Fa-f0-9]{16})\}"#).unwrap();
        if let Some(caps) = re_guid.captures(&self.imageset.texture_path_1x) {
            return caps.get(1).map_or("", |m| m.as_str()).to_string();
        }

        // สุ่ม 64-bit Hex GUID 16 หลัก
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let new_guid = format!("{:016X}", (seed ^ 0x534691EE0479871C) as u64);
        
        // ผูก GUID ใหม่เข้ากับ path ทันที
        let clean_path = self.imageset.texture_path_1x.trim_start_matches('{');
        let clean_path = if let Some(idx) = clean_path.find('}') {
            &clean_path[idx + 1..]
        } else {
            clean_path
        };
        self.imageset.texture_path_1x = format!("{{{}}}{}", new_guid, clean_path);
        self.update_code();
        new_guid
    }

    // ส่งออกและแปลงภาพเป็น .edds พร้อมสร้างไฟล์ .meta อัตโนมัติ
    fn export_edds_and_meta(&mut self, output_edds_path: &Path) -> Result<String, String> {
        let guid = self.get_or_generate_guid();
        let edds_filename = output_edds_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let stem = output_edds_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let parent_dir = output_edds_path.parent().unwrap_or_else(|| Path::new("."));

        // 1. เตรียมภาพ PNG ชั่วคราวที่จะนำไปแปลงเป็น EDDS
        let temp_png_path = parent_dir.join(format!("__temp_export_{}.png", stem));
        let png_to_convert = if let Some(ref img) = self.loaded_rgba_image {
            img.save(&temp_png_path).map_err(|e| format!("Failed to save temp PNG: {}", e))?;
            temp_png_path.clone()
        } else if let Some(ref path) = self.loaded_texture_path {
            if path.extension().unwrap_or_default().to_string_lossy().to_lowercase() == "png" {
                path.clone()
            } else {
                return Err("No active image buffer found to convert to EDDS".to_string());
            }
        } else {
            return Err("Please load a texture image before exporting EDDS".to_string());
        };

        // 2. ดึง imagesetconv tool
        let conv_tool = Self::get_imagesetconv_tool()
            .ok_or_else(|| "imagesetconv tool could not be loaded!".to_string())?;

        // 3. เรียกคำสั่ง png2edds
        let mut cmd = Command::new(&conv_tool);
        cmd.args(&["png2edds", "-png", &png_to_convert.to_string_lossy(), "-output", &output_edds_path.to_string_lossy()]);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute imagesetconv: {}", e))?;

        if temp_png_path.exists() {
            let _ = fs::remove_file(&temp_png_path);
        }

        if !output.status.success() || !output_edds_path.exists() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("png2edds conversion failed: {}", err_msg));
        }

        // 4. สร้างเนื้อหาไฟล์ .edds.meta
        let clean_path = self.imageset.texture_path_1x.replace(&format!("{{{}}}", guid), "");
        let virtual_res_path = if clean_path.trim().is_empty() {
            format!("Gui/imagesets/{}", edds_filename)
        } else {
            clean_path
        };

        let meta_content = format!(
            "MetaFileClass {{\n Name \"{{{guid}}}{virtual_res_path}\"\n Configurations {{\n  PNGResourceClass PC {{\n   SourceFile \"{stem}.png\"\n  }}\n  TGAResourceClass XBOX_ONE : PC {{\n  }}\n  TGAResourceClass PS4 : PC {{\n  }}\n  TGAResourceClass LINUX : PC {{\n  }}\n }}\n}}\n",
            guid = guid,
            virtual_res_path = virtual_res_path,
            stem = stem
        );

        let meta_path = PathBuf::from(format!("{}.meta", output_edds_path.display()));
        fs::write(&meta_path, meta_content).map_err(|e| format!("Failed to write .meta file: {}", e))?;

        Ok(format!("Successfully exported: {} and {}.meta (GUID: {})", edds_filename, edds_filename, guid))
    }

    fn load_texture_from_file(&mut self, ctx: &egui::Context, path: &Path) {
        let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
        
        // รองรับการแปลงไฟล์ .edds ให้เป็น .png เพื่อแสดงผลบน Canvas
        let target_path = if ext == "edds" {
            if let Some(converted) = self.convert_edds_to_png(path) {
                converted
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

        match image::open(&target_path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                
                self.texture_handle = Some(ctx.load_texture(
                    path.to_string_lossy(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
                self.loaded_texture_path = Some(path.to_path_buf());
                self.loaded_rgba_image = Some(rgba.clone());
                
                self.imageset.ref_size = (rgba.width(), rgba.height());
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy().to_string();
                    if self.imageset.name == "dayz_gui" || self.imageset.name == "MyMod_Icons" {
                        self.imageset.name = name.clone();
                        
                        // ตรวจสอบว่ามี .edds.meta หรือไม่ เพื่อนำ GUID มาใส่ใน path
                        if let Some((guid, v_path)) = self.find_meta_guid_and_path(path) {
                            self.imageset.texture_path_1x = format!("{{{}}}{}", guid, v_path);
                        } else {
                            self.imageset.texture_path_1x = format!("Gui/imagesets/{}.edds", name);
                        }

                        // ตรวจสอบไฟล์ @2x คู่ขนาน
                        let stem_2x = format!("{}@2x.edds", name);
                        let path_2x = path.with_file_name(&stem_2x);
                        if path_2x.exists() {
                            if let Some((guid_2x, v_path_2x)) = self.find_meta_guid_and_path(&path_2x) {
                                self.imageset.texture_path_2x = format!("{{{}}}{}", guid_2x, v_path_2x);
                            } else {
                                self.imageset.texture_path_2x = format!("Gui/imagesets/{}.edds", stem_2x);
                            }
                            self.imageset.enable_2x = true;
                        } else {
                            self.imageset.texture_path_2x = format!("Gui/imagesets/{}@2x.edds", name);
                        }
                    }
                }
                self.status_msg = format!("Loaded {} texture: {} ({}x{} px)", ext.to_uppercase(), path.file_name().unwrap_or_default().to_string_lossy(), rgba.width(), rgba.height());
                self.update_code();
            }
            Err(e) => {
                self.status_msg = format!("Failed to open image: {}", e);
            }
        }
    }

    fn import_imageset_from_path(&mut self, ctx: &egui::Context, path: &Path) {
        if let Ok(content) = fs::read_to_string(path) {
            self.save_undo_state();
            self.imageset = parse_imageset(&content);
            self.selected_index = if !self.imageset.images.is_empty() { Some(0) } else { None };
            self.selected_indices.clear();
            if let Some(idx) = self.selected_index {
                self.selected_indices.insert(idx);
            }
            self.status_msg = format!("Imported: {} sprite(s) from {}", self.imageset.images.len(), path.file_name().unwrap_or_default().to_string_lossy());
            self.update_code();

            // ค้นหาและโหลด Texture (.edds หรือ .png) ตาม path ที่ระบุใน .imageset โดยอัตโนมัติ
            let re_guid_strip = Regex::new(r#"^\{[A-Fa-f0-9]+\}"#).unwrap();
            let clean_path = re_guid_strip.replace(&self.imageset.texture_path_1x, "").to_string();
            let texture_rel = clean_path.replace('\\', "/");
            let filename = Path::new(&texture_rel).file_name().unwrap_or_default().to_string_lossy().to_string();
            
            let mut texture_candidates = Vec::new();
            // 1. โฟลเดอร์เดียวกันกับไฟล์ .imageset
            if let Some(parent) = path.parent() {
                texture_candidates.push(parent.join(&filename));
                texture_candidates.push(parent.join(Path::new(&filename).with_extension("png")));
                texture_candidates.push(parent.join(Path::new(&filename).with_extension("edds")));
            }
            // 2. หาจาก Drive Root หรือตาม Relative path (เช่น P:\Gui\imagesets\...)
            texture_candidates.push(PathBuf::from(&texture_rel));
            texture_candidates.push(PathBuf::from(format!("P:/{}", texture_rel)));
            texture_candidates.push(PathBuf::from(format!("P:/gui/imagesets/{}", filename)));

            for cand in texture_candidates {
                if cand.exists() {
                    let mut preserve_1x = self.imageset.texture_path_1x.clone();
                    let mut preserve_2x = self.imageset.texture_path_2x.clone();
                    let mut preserve_en2x = self.imageset.enable_2x;
                    
                    // ตรวจสอบว่าใน .imageset เดิมมี GUID หรือไม่ ถ้าไม่มีให้ลองหาจาก .meta
                    if !preserve_1x.starts_with('{') {
                        if let Some((meta_guid, v_path)) = self.find_meta_guid_and_path(&cand) {
                            preserve_1x = format!("{{{}}}{}", meta_guid, v_path);
                        }
                    }

                    let cand_stem = cand.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let cand_2x = cand.with_file_name(format!("{}@2x.edds", cand_stem));
                    if cand_2x.exists() && !preserve_2x.starts_with('{') {
                        if let Some((guid_2x, v_path_2x)) = self.find_meta_guid_and_path(&cand_2x) {
                            preserve_2x = format!("{{{}}}{}", guid_2x, v_path_2x);
                            preserve_en2x = true;
                        }
                    }

                    self.load_texture_from_file(ctx, &cand);
                    
                    // รักษารูปแบบ Texture Path (รวมถึง {GUID}) ที่อ่านมาจาก .imageset เดิมหรือดึงจาก .meta
                    self.imageset.texture_path_1x = preserve_1x;
                    self.imageset.texture_path_2x = preserve_2x;
                    self.imageset.enable_2x = preserve_en2x;
                    self.update_code();
                    break;
                }
            }
        }
    }
}

impl eframe::App for ImagesetEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // กำหนดขนาดตัวอักษรเริ่มต้นของ UI ให้ใหญ่และอ่านง่ายขึ้น
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.5, egui::FontFamily::Proportional),
        );
        ctx.set_style(style);

        // รองรับการลากไฟล์มาวางในหน้าต่างโปรแกรม (Drag & Drop Files)
        let dropped_files = ctx.input().raw.dropped_files.clone();
        if !dropped_files.is_empty() {
            for dropped in dropped_files {
                if let Some(path) = dropped.path {
                    let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
                    if ext == "imageset" || ext == "txt" {
                        self.import_imageset_from_path(ctx, &path);
                    } else if ["edds", "png", "tga", "jpg", "jpeg", "webp", "bmp"].contains(&ext.as_str()) {
                        self.load_texture_from_file(ctx, &path);
                    }
                }
            }
        }

        // จัดการคีย์ลัดเมื่อไม่ได้โฟกัสพิมพ์ในช่องข้อความ
        if !ctx.wants_keyboard_input() {
            let modifiers = ctx.input().modifiers;

            // Undo (Ctrl+Z) & Redo (Ctrl+Y / Ctrl+Shift+Z)
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::Z) {
                if modifiers.shift {
                    self.redo();
                } else {
                    self.undo();
                }
            }
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::Y) {
                self.redo();
            }

            // Select All (Ctrl+A), Duplicate (Ctrl+D), Copy (Ctrl+C), Paste (Ctrl+V)
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::A) {
                self.select_all_sprites();
            }
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::D) {
                self.duplicate_selected_box();
            }
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::C) {
                self.copy_selected_sprites();
            }
            if modifiers.ctrl && ctx.input().key_pressed(egui::Key::V) {
                self.paste_sprites();
            }

            // Delete (Delete / Backspace)
            if ctx.input().key_pressed(egui::Key::Delete) || ctx.input().key_pressed(egui::Key::Backspace) {
                self.delete_selected_box();
            }

            // Arrow Keys Nudge (1px ปกติ หรือตาม Grid Size เมื่อกด Shift ค้าง)
            let step = if modifiers.shift { self.grid_size.max(1) } else { 1 };
            if ctx.input().key_pressed(egui::Key::ArrowLeft) {
                self.nudge_selected(-step, 0);
            }
            if ctx.input().key_pressed(egui::Key::ArrowRight) {
                self.nudge_selected(step, 0);
            }
            if ctx.input().key_pressed(egui::Key::ArrowUp) {
                self.nudge_selected(0, -step);
            }
            if ctx.input().key_pressed(egui::Key::ArrowDown) {
                self.nudge_selected(0, step);
            }
        }

        // Load embedded brand icon for UI if not loaded yet
        if self.brand_icon.is_none() {
            let icon_bytes = include_bytes!("../app_icon.png");
            if let Ok(img) = image::load_from_memory(icon_bytes) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.as_flat_samples();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.brand_icon = Some(ctx.load_texture("brand_icon", color_image, egui::TextureOptions::LINEAR));
            }
        }

        // --- 1. Top Header Bar (Modern Dark Bar with clear action buttons) ---
        egui::TopBottomPanel::top("top_panel")
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 24, 32))
                    .inner_margin(egui::style::Margin::symmetric(14.0, 10.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 56, 70)))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                    ui.spacing_mut().button_padding = egui::vec2(12.0, 7.0);

                    if let Some(ref icon_tex) = self.brand_icon {
                        ui.image(icon_tex.id(), egui::vec2(26.0, 26.0));
                    }

                    // --- New Project Button ---
                    let new_btn = egui::Button::new(
                        egui::RichText::new("📄 New Project").size(13.5).strong().color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(55, 65, 81));

                    if ui.add(new_btn).on_hover_text("Reset all sprites and create a brand new imageset project").clicked() {
                        self.new_project();
                    }

                    // Primary Action: Load Texture (EDDS / PNG / TGA / JPG / WEBP)
                    let load_btn = egui::Button::new(
                        egui::RichText::new("🖼 Load Texture (EDDS/PNG)").size(13.5).strong().color(egui::Color32::BLACK)
                    ).fill(egui::Color32::from_rgb(217, 119, 6));

                    if ui.add(load_btn).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Texture Image (*.edds, *.png, *.tga, *.jpg)", &["edds", "png", "tga", "jpg", "jpeg", "webp", "bmp"])
                            .pick_file()
                        {
                            self.load_texture_from_file(ctx, &path);
                        }
                    }

                    let import_btn = egui::Button::new(
                        egui::RichText::new("📂 Import .imageset").size(13.5).color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(37, 99, 235));

                    if ui.add(import_btn).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("ImageSet (*.imageset, *.txt)", &["imageset", "txt"])
                            .pick_file()
                        {
                            self.import_imageset_from_path(ctx, &path);
                        }
                    }

                    // Primary Action: Save .imageset
                    let save_btn = egui::Button::new(
                        egui::RichText::new("💾 Save .imageset").size(13.5).strong().color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(16, 185, 129));

                    if ui.add(save_btn).clicked() {
                        let default_name = format!("{}.imageset", self.imageset.name);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&default_name)
                            .add_filter("ImageSet (*.imageset)", &["imageset"])
                            .save_file()
                        {
                            if fs::write(&path, &self.code_cache).is_ok() {
                                self.status_msg = format!("Saved file: {}", path.display());
                            } else {
                                self.status_msg = "Error saving file!".to_string();
                            }
                        }
                    }

                    // Export .edds and .meta in 1-Click
                    let export_edds_btn = egui::Button::new(
                        egui::RichText::new("📦 Export EDDS + .meta").size(13.5).strong().color(egui::Color32::WHITE)
                    ).fill(egui::Color32::from_rgb(147, 51, 234));

                    if ui.add(export_edds_btn).on_hover_text("ส่งออกภาพเป็น .edds และสร้างไฟล์ .edds.meta พร้อมลงทะเบียน GUID อัตโนมัติ").clicked() {
                        let default_edds_name = format!("{}.edds", self.imageset.name);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&default_edds_name)
                            .add_filter("Enfusion Texture (*.edds)", &["edds"])
                            .save_file()
                        {
                            match self.export_edds_and_meta(&path) {
                                Ok(msg) => self.status_msg = msg,
                                Err(err) => self.status_msg = format!("Export Error: {}", err),
                            }
                        }
                    }

                    ui.separator();

                    // ปุ่ม Undo / Redo บนแถบเมนูหลัก
                    let can_undo = !self.undo_stack.is_empty();
                    let undo_btn = egui::Button::new(
                        egui::RichText::new("↩ Undo").size(13.0)
                    ).fill(if can_undo { egui::Color32::from_rgb(45, 55, 72) } else { egui::Color32::from_rgb(28, 34, 44) });
                    if ui.add_enabled(can_undo, undo_btn).on_hover_text("ย้อนกลับการแก้ไขล่าสุด (Ctrl+Z)").clicked() {
                        self.undo();
                    }

                    let can_redo = !self.redo_stack.is_empty();
                    let redo_btn = egui::Button::new(
                        egui::RichText::new("↪ Redo").size(13.0)
                    ).fill(if can_redo { egui::Color32::from_rgb(45, 55, 72) } else { egui::Color32::from_rgb(28, 34, 44) });
                    if ui.add_enabled(can_redo, redo_btn).on_hover_text("ทำซ้ำการแก้ไข (Ctrl+Y)").clicked() {
                        self.redo();
                    }

                    // ด้านขวาสุดแสดง Version ของ Application และสถานะ
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("v0.2.0")
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::from_rgb(245, 158, 11))
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(&self.status_msg)
                                .size(12.0)
                                .color(egui::Color32::from_rgb(139, 148, 158))
                        );
                    });
                });
            });

        // --- 2. Left Side Panel: ImageSet Config & Sprite List ---
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(310.0)
            .width_range(260.0..=460.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 24, 32))
                    .inner_margin(egui::style::Margin::same(16.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 56, 70)))
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("1. ImageSet Properties")
                        .strong()
                        .size(14.0)
                        .color(egui::Color32::from_rgb(240, 246, 252))
                );
                ui.add_space(8.0);

                let mut changed = false;

                // Name field
                ui.label(egui::RichText::new("ImageSet Name:").size(12.5));
                if ui.add(
                    egui::TextEdit::singleline(&mut self.imageset.name)
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(8.0, 6.0))
                ).changed() {
                    changed = true;
                }
                ui.add_space(8.0);

                // Texture path mpix 0
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Texture (mpix 0) .edds:").size(12.5));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("(Ctrl+V to paste GUID)").size(11.0).color(egui::Color32::from_rgb(139, 148, 158)));
                    });
                });

                if ui.add(
                    egui::TextEdit::singleline(&mut self.imageset.texture_path_1x)
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(8.0, 6.0))
                        .hint_text("{GUID}Gui/imagesets/name.edds")
                ).changed() {
                    changed = true;
                }
                ui.add_space(8.0);

                // Optional mpix 1 (@2x) texture
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.imageset.enable_2x, egui::RichText::new("Enable 2x Texture (mpix 1):").size(12.5)).changed() {
                        changed = true;
                    }
                });

                if self.imageset.enable_2x {
                    if ui.add(
                        egui::TextEdit::singleline(&mut self.imageset.texture_path_2x)
                            .desired_width(f32::INFINITY)
                            .margin(egui::vec2(8.0, 6.0))
                            .hint_text("{GUID}Gui/imagesets/name@2x.edds")
                    ).changed() {
                        changed = true;
                    }
                }
                ui.add_space(8.0);

                // RefSize W x H
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Ref Width:").size(12.0));
                        let mut w_str = self.imageset.ref_size.0.to_string();
                        if ui.add(
                            egui::TextEdit::singleline(&mut w_str)
                                .desired_width(120.0)
                                .margin(egui::vec2(8.0, 6.0))
                        ).changed() {
                            if let Ok(w) = w_str.parse::<u32>() { self.imageset.ref_size.0 = w; changed = true; }
                        }
                    });

                    ui.add_space(10.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Ref Height:").size(12.0));
                        let mut h_str = self.imageset.ref_size.1.to_string();
                        if ui.add(
                            egui::TextEdit::singleline(&mut h_str)
                                .desired_width(120.0)
                                .margin(egui::vec2(8.0, 6.0))
                        ).changed() {
                            if let Ok(h) = h_str.parse::<u32>() { self.imageset.ref_size.1 = h; changed = true; }
                        }
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);

                // Section 2: Sprites List Header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("2. Sprites ({})", self.imageset.images.len()))
                            .strong()
                            .size(14.0)
                            .color(egui::Color32::from_rgb(240, 246, 252))
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let add_btn = egui::Button::new(
                            egui::RichText::new("+ Add Box").size(12.0).strong()
                        ).fill(egui::Color32::from_rgb(45, 55, 72));

                        if ui.add(add_btn).on_hover_text("เพิ่มกล่อง Sprite ใหม่ที่มุมบนซ้าย").clicked() {
                            self.save_undo_state();
                            let next_num = self.imageset.images.len() + 1;
                            self.imageset.images.push(ImageDef {
                                id: format!("icon_{}", next_num),
                                name: format!("icon_{}", next_num),
                                pos: (16, 16),
                                size: (64, 64),
                                flags: "0".to_string(),
                                group: None,
                            });
                            let new_idx = self.imageset.images.len() - 1;
                            self.selected_index = Some(new_idx);
                            self.selected_indices.clear();
                            self.selected_indices.insert(new_idx);
                            changed = true;
                        }

                        // ปุ่ม Auto-Detect All Icons
                        let has_tex = self.loaded_rgba_image.is_some();
                        let detect_btn = egui::Button::new(
                            egui::RichText::new("🔍 Auto-Detect").size(12.0).strong()
                        ).fill(if has_tex { egui::Color32::from_rgb(13, 148, 136) } else { egui::Color32::from_rgb(30, 41, 59) });

                        if ui.add_enabled(has_tex, detect_btn).on_hover_text("สแกนหาไอคอนทั้งหมดบนแผ่นภาพอัตโนมัติ (Alpha Island Detection)").clicked() {
                            self.auto_detect_all_icons();
                            changed = true;
                        }
                    });
                });
                ui.add_space(6.0);

                // ช่องค้นหา Sprite (Search / Filter Box)
                ui.horizontal(|ui| {
                    let is_search_empty = self.search_query.is_empty();
                    let search_w = if is_search_empty { f32::INFINITY } else { ui.available_width() - 32.0 };
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("🔍 Search sprite name or group...")
                            .desired_width(search_w)
                            .margin(egui::vec2(8.0, 5.0))
                    );
                    if !self.search_query.is_empty() {
                        if ui.button("✕").on_hover_text("Clear search").clicked() {
                            self.search_query.clear();
                        }
                    }
                });
                ui.add_space(8.0);

                // Sprites List Body
                let query_lower = self.search_query.trim().to_lowercase();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.imageset.images.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("No sprite boxes yet.\nDrag on canvas, click '+ Add Box', or '🔍 Auto-Detect'")
                                    .size(12.5)
                                    .color(egui::Color32::from_rgb(100, 115, 130))
                            );
                        });
                    }

                    let mut to_delete = None;
                    let mut focus_clicked = None;
                    for (idx, img) in self.imageset.images.iter().enumerate() {
                        if !query_lower.is_empty() {
                            let match_name = img.name.to_lowercase().contains(&query_lower);
                            let match_grp = img.group.as_ref().map(|g| g.to_lowercase().contains(&query_lower)).unwrap_or(false);
                            if !match_name && !match_grp {
                                continue;
                            }
                        }

                        let is_sel = self.selected_index == Some(idx) || self.selected_indices.contains(&idx);
                        let bg_color = if is_sel {
                            egui::Color32::from_rgb(42, 54, 70)
                        } else {
                            egui::Color32::from_rgb(24, 30, 38)
                        };

                        let mut del_clicked = false;
                        let mut jump_clicked = false;
                        let item_frame = egui::Frame::none()
                            .fill(bg_color)
                            .inner_margin(egui::style::Margin::symmetric(12.0, 10.0))
                            .stroke(egui::Stroke::new(1.0_f32, if is_sel { egui::Color32::from_rgb(245, 158, 11) } else { egui::Color32::from_rgb(40, 48, 60) }));

                        let frame_resp = item_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let text = format!("{}. {}", idx + 1, img.name);
                                let subtext = format!("Pos: {}, {} | Size: {}x{}", img.pos.0, img.pos.1, img.size.0, img.size.1);
                                
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(text).strong().size(13.0).color(if is_sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(220, 230, 242) }));
                                    ui.add_space(2.0);
                                    ui.label(egui::RichText::new(subtext).size(11.0).color(egui::Color32::from_rgb(139, 148, 158)));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let del_btn = egui::Button::new(
                                        egui::RichText::new("✕").size(13.0).color(egui::Color32::from_rgb(248, 113, 113))
                                    );
                                    if ui.add(del_btn).on_hover_text("Delete this sprite box").clicked() {
                                        del_clicked = true;
                                    }

                                    let focus_btn = egui::Button::new(
                                        egui::RichText::new("🎯").size(12.5)
                                    );
                                    if ui.add(focus_btn).on_hover_text("เลื่อนมุมมอง Canvas ไปหากรอบนี้ (Focus)").clicked() {
                                        jump_clicked = true;
                                    }
                                });
                            });
                        });

                        let card_interact = ui.interact(frame_resp.response.rect, egui::Id::new(("sprite_card", idx)), egui::Sense::click());
                        if del_clicked {
                            to_delete = Some(idx);
                        } else if jump_clicked || card_interact.double_clicked() {
                            self.selected_indices.clear();
                            self.selected_indices.insert(idx);
                            self.selected_index = Some(idx);
                            focus_clicked = Some(idx);
                        } else if card_interact.clicked() {
                            // คลิกการ์ด: รองรับ Ctrl/Shift สำหรับ Multi-selection
                            let modifiers = ctx.input().modifiers;
                            if modifiers.ctrl || modifiers.shift {
                                if self.selected_indices.contains(&idx) {
                                    self.selected_indices.remove(&idx);
                                } else {
                                    self.selected_indices.insert(idx);
                                }
                                self.selected_index = Some(idx);
                            } else {
                                self.selected_indices.clear();
                                self.selected_indices.insert(idx);
                                self.selected_index = Some(idx);
                            }
                        }

                        ui.add_space(6.0);
                    }

                    if let Some(f_idx) = focus_clicked {
                        self.focus_target_index = Some(f_idx);
                    }

                    if let Some(idx) = to_delete {
                        self.save_undo_state();
                        self.imageset.images.remove(idx);
                        self.selected_indices.remove(&idx);
                        self.selected_index = if !self.imageset.images.is_empty() {
                            Some(idx.saturating_sub(1))
                        } else {
                            None
                        };
                        changed = true;
                    }
                });

                if changed {
                    self.update_code();
                }
            });

        // --- 3. Right Side Panel: Inspector & Generated Code ---
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(330.0)
            .width_range(280.0..=520.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 24, 32))
                    .inner_margin(egui::style::Margin::same(16.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 56, 70)))
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("3. Sprite Inspector")
                        .strong()
                        .size(14.0)
                        .color(egui::Color32::from_rgb(240, 246, 252))
                );
                ui.add_space(10.0);

                let mut changed = false;
                let mut action_auto_trim = false;
                let mut action_duplicate = false;
                let mut action_delete = false;
                let mut action_copy_enforce: Option<String> = None;

                if let Some(idx) = self.selected_index {
                    if idx < self.imageset.images.len() {
                        let set_name = self.imageset.name.clone();
                        let sp = &mut self.imageset.images[idx];

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Sprite Name:").size(12.5));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let enforce_btn = egui::Button::new(
                                    egui::RichText::new("📋 Copy set:...").size(11.5).strong().color(egui::Color32::from_rgb(167, 243, 208))
                                ).fill(egui::Color32::from_rgb(6, 78, 59));
                                if ui.add(enforce_btn).on_hover_text("คัดลอกพาธ Enforce Script: set:<imageset> image:<sprite>").clicked() {
                                    action_copy_enforce = Some(format!("set:{} image:{}", set_name, sp.name));
                                }
                            });
                        });
                        if ui.add(
                            egui::TextEdit::singleline(&mut sp.name)
                                .desired_width(f32::INFINITY)
                                .margin(egui::vec2(8.0, 6.0))
                        ).changed() {
                            changed = true;
                        }
                        ui.add_space(8.0);

                        // Coordinates in 2 columns
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Pos X (Left):").size(12.0));
                                let mut x = sp.pos.0.to_string();
                                if ui.add(
                                    egui::TextEdit::singleline(&mut x)
                                        .desired_width(110.0)
                                        .margin(egui::vec2(8.0, 6.0))
                                ).changed() {
                                    if let Ok(val) = x.parse::<i32>() { sp.pos.0 = val; changed = true; }
                                }
                            });

                            ui.add_space(10.0);

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Pos Y (Top):").size(12.0));
                                let mut y = sp.pos.1.to_string();
                                if ui.add(
                                    egui::TextEdit::singleline(&mut y)
                                        .desired_width(110.0)
                                        .margin(egui::vec2(8.0, 6.0))
                                ).changed() {
                                    if let Ok(val) = y.parse::<i32>() { sp.pos.1 = val; changed = true; }
                                }
                            });
                        });
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Size W (Width):").size(12.0));
                                let mut w = sp.size.0.to_string();
                                if ui.add(
                                    egui::TextEdit::singleline(&mut w)
                                        .desired_width(110.0)
                                        .margin(egui::vec2(8.0, 6.0))
                                ).changed() {
                                    if let Ok(val) = w.parse::<u32>() { sp.size.0 = val; changed = true; }
                                }
                            });

                            ui.add_space(10.0);

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Size H (Height):").size(12.0));
                                let mut h = sp.size.1.to_string();
                                if ui.add(
                                    egui::TextEdit::singleline(&mut h)
                                        .desired_width(110.0)
                                        .margin(egui::vec2(8.0, 6.0))
                                ).changed() {
                                    if let Ok(val) = h.parse::<u32>() { sp.size.1 = val; changed = true; }
                                }
                            });
                        });
                        ui.add_space(8.0);

                        // Flags field with presets (0, ISVerticalTile, ISHorizontalTile, 3)
                        ui.label(egui::RichText::new("Flags (Enum / Number):").size(12.0));
                        ui.horizontal(|ui| {
                            if ui.add(
                                egui::TextEdit::singleline(&mut sp.flags)
                                    .desired_width(120.0)
                                    .margin(egui::vec2(8.0, 5.0))
                            ).changed() {
                                changed = true;
                            }

                            // Quick preset pills
                            ui.spacing_mut().button_padding = egui::vec2(7.0, 4.0);
                            if ui.button("0").on_hover_text("Standard 0").clicked() {
                                sp.flags = "0".to_string();
                                changed = true;
                            }
                            if ui.button("VTile").on_hover_text("ISVerticalTile").clicked() {
                                sp.flags = "ISVerticalTile".to_string();
                                changed = true;
                            }
                            if ui.button("HTile").on_hover_text("ISHorizontalTile").clicked() {
                                sp.flags = "ISHorizontalTile".to_string();
                                changed = true;
                            }
                            if ui.button("3").on_hover_text("Grid Center 3").clicked() {
                                sp.flags = "3".to_string();
                                changed = true;
                            }
                        });
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Group (Optional):").size(12.0));
                                let mut grp = sp.group.clone().unwrap_or_default();
                                if ui.add(
                                    egui::TextEdit::singleline(&mut grp)
                                        .desired_width(f32::INFINITY)
                                        .margin(egui::vec2(8.0, 6.0))
                                ).changed() {
                                    sp.group = if grp.trim().is_empty() { None } else { Some(grp) };
                                    changed = true;
                                }
                            });
                        });

                        ui.add_space(12.0);

                        // ปุ่ม 🪄 Auto-Trim Box (หดกรอบให้พอดีขอบไอคอนอัตโนมัติ)
                        let trim_btn = egui::Button::new(
                            egui::RichText::new("🪄 Auto-Trim to Icon Pixels").size(12.5).strong().color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(13, 148, 136));
                        if ui.add_sized([ui.available_width(), 30.0], trim_btn)
                            .on_hover_text("สแกนหาพิกเซลที่ไม่โปร่งใส (Alpha > 10) เพื่อหดกรอบให้พอดีกับตัวไอคอนอัตโนมัติ")
                            .clicked()
                        {
                            action_auto_trim = true;
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().button_padding = egui::vec2(12.0, 7.0);

                            // ปุ่ม Duplicate Box ใน Inspector
                            let dup_btn = egui::Button::new(
                                egui::RichText::new("📋 Duplicate (Ctrl+D)").size(12.5).strong()
                            ).fill(egui::Color32::from_rgb(37, 99, 235));

                            if ui.add(dup_btn).on_hover_text("ทำสำเนากล่องนี้พร้อมขนาดและค่าต่างๆ ไปยังตำแหน่งถัดไป").clicked() {
                                action_duplicate = true;
                            }

                            // ปุ่ม Delete Box ใน Inspector
                            let del_btn = egui::Button::new(
                                egui::RichText::new("🗑 Delete Box (Del)").size(12.5).strong()
                            ).fill(egui::Color32::from_rgb(220, 38, 38));

                            if ui.add(del_btn).on_hover_text("ลบกล่อง Sprite นี้ (กดปุ่ม Delete หรือ Backspace)").clicked() {
                                action_delete = true;
                            }
                        });
                    }
                } else {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(16, 20, 26))
                        .inner_margin(egui::style::Margin::same(14.0))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(34, 42, 54)))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("No sprite selected.\nClick any box on canvas to inspect/edit.")
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(139, 148, 158))
                            );
                        });
                }

                if let Some(snippet) = action_copy_enforce {
                    ui.output().copied_text = snippet.clone();
                    self.status_msg = format!("Copied Enforce snippet: {}", snippet);
                }
                if action_auto_trim {
                    self.auto_trim_selected_box();
                }
                if action_duplicate {
                    self.duplicate_selected_box();
                }
                if action_delete {
                    self.delete_selected_box();
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);

                // Section 4: Live Code Header with Proper Padding and Clean Container
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("4. Live .imageset Code")
                            .strong()
                            .size(14.0)
                            .color(egui::Color32::from_rgb(240, 246, 252))
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let copy_btn = egui::Button::new(
                            egui::RichText::new("📋 Copy Code").size(12.0)
                        ).fill(egui::Color32::from_rgb(45, 55, 72));

                        if ui.add(copy_btn).clicked() {
                            ui.output().copied_text = self.code_cache.clone();
                            self.status_msg = "Copied imageset code to clipboard!".to_string();
                        }
                    });
                });
                ui.add_space(8.0);

                // Nicely Padded & Bordered Code Preview Box
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(13, 17, 23))
                    .inner_margin(egui::style::Margin::same(12.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(40, 50, 65)))
                    .rounding(egui::Rounding::same(6.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.code_cache)
                                        .font(egui::TextStyle::Monospace)
                                        .text_color(egui::Color32::from_rgb(140, 205, 255))
                                        .frame(false)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(22),
                                );
                            });
                    });

                if changed {
                    self.update_code();
                }
            });

        // --- 4. Center Stage: Interactive Visual Canvas ---
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(11, 14, 18)))
            .show(ctx, |ui| {
                // Secondary Toolbar directly above canvas
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(18, 22, 28))
                    .inner_margin(egui::style::Margin::symmetric(14.0, 10.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 46, 58)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
                            ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);

                            // Tool Mode Selector (Draw/Edit vs Pan)
                            let is_draw = self.tool_mode == ToolMode::DrawEdit;
                            let is_pan = self.tool_mode == ToolMode::Pan;

                            let draw_btn = egui::Button::new(
                                egui::RichText::new("✏ Draw / Edit Mode").size(13.0).strong()
                            ).fill(if is_draw { egui::Color32::from_rgb(37, 99, 235) } else { egui::Color32::from_rgb(26, 32, 40) });
                            if ui.add(draw_btn).clicked() {
                                self.tool_mode = ToolMode::DrawEdit;
                            }

                            let pan_btn = egui::Button::new(
                                egui::RichText::new("✋ Pan Mode").size(13.0).strong()
                            ).fill(if is_pan { egui::Color32::from_rgb(37, 99, 235) } else { egui::Color32::from_rgb(26, 32, 40) });
                            if ui.add(pan_btn).clicked() {
                                self.tool_mode = ToolMode::Pan;
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Snap Grid controls
                            ui.checkbox(&mut self.snap_grid, egui::RichText::new("Snap Grid:").size(13.0));
                            ui.add(egui::Slider::new(&mut self.grid_size, 1..=64).suffix(" px"));

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Zoom controls
                            let zoom_out = egui::Button::new(egui::RichText::new(" ➖ ").size(11.0));
                            if ui.add(zoom_out).clicked() { self.zoom = (self.zoom / 1.15_f32).max(0.1_f32); }
                            
                            ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).strong().size(13.0).monospace());
                            
                            let zoom_in = egui::Button::new(egui::RichText::new(" ➕ ").size(11.0));
                            if ui.add(zoom_in).clicked() { self.zoom = (self.zoom * 1.15_f32).min(8.0_f32); }

                            let fit_btn = egui::Button::new(
                                egui::RichText::new("⛶ Fit / Reset View").size(13.0)
                            ).fill(egui::Color32::from_rgb(45, 55, 72));
                            if ui.add(fit_btn).clicked() {
                                self.pan = egui::vec2(60.0, 60.0);
                                self.zoom = 0.65;
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Toggle Display Sprite Names Checkbox
                            ui.checkbox(&mut self.show_labels, egui::RichText::new("🏷 Show Names").size(13.0));

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            let has_sel = self.selected_index.is_some();
                            let trim_tb = egui::Button::new(
                                egui::RichText::new("🪄 Auto-Trim").size(12.5).strong()
                            ).fill(if has_sel { egui::Color32::from_rgb(13, 148, 136) } else { egui::Color32::from_rgb(28, 34, 44) });
                            if ui.add_enabled(has_sel, trim_tb).on_hover_text("หดกรอบให้พอดีขอบไอคอนอัตโนมัติ (Alpha Trim)").clicked() {
                                self.auto_trim_selected_box();
                            }
                        });
                    });

                // Canvas area allocation
                let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

                // เลื่อน Canvas ไปหากล่องที่กด Focus (🎯) หรือดับเบิลคลิกในรายการ
                if let Some(f_idx) = self.focus_target_index.take() {
                    if f_idx < self.imageset.images.len() {
                        let sp = &self.imageset.images[f_idx];
                        let sp_center_x = sp.pos.0 as f32 + (sp.size.0 as f32 * 0.5);
                        let sp_center_y = sp.pos.1 as f32 + (sp.size.1 as f32 * 0.5);
                        let view_half = response.rect.size() * 0.5;
                        self.pan = egui::vec2(
                            view_half.x - sp_center_x * self.zoom,
                            view_half.y - sp_center_y * self.zoom,
                        );
                        self.status_msg = format!("🎯 Focused on sprite: {}", sp.name);
                    }
                }

                let canvas_origin = response.rect.min + self.pan;

                // Zoom with Scroll Wheel
                let scroll_delta = ctx.input().scroll_delta.y;
                if scroll_delta != 0.0 && response.hovered() {
                    let zoom_factor = if scroll_delta > 0.0 { 1.15_f32 } else { 0.85_f32 };
                    self.zoom = (self.zoom * zoom_factor).clamp(0.1_f32, 8.0_f32);
                }

                // Panning logic: Pan tool OR Middle mouse OR Right mouse
                let is_panning = self.tool_mode == ToolMode::Pan 
                    || response.dragged_by(egui::PointerButton::Middle) 
                    || response.dragged_by(egui::PointerButton::Secondary);

                if is_panning {
                    self.pan += response.drag_delta();
                }

                // Coordinate conversion helpers
                let current_zoom = self.zoom;
                let to_screen = |x: f32, y: f32| -> egui::Pos2 {
                    canvas_origin + egui::vec2(x * current_zoom, y * current_zoom)
                };
                let to_world = |p: egui::Pos2| -> (f32, f32) {
                    let offset = p - canvas_origin;
                    (offset.x / current_zoom, offset.y / current_zoom)
                };

                let ref_w = self.imageset.ref_size.0 as f32;
                let ref_h = self.imageset.ref_size.1 as f32;
                let atlas_rect = egui::Rect::from_min_max(to_screen(0.0, 0.0), to_screen(ref_w, ref_h));

                // 1. Draw Checkerboard background for texture area
                let tile_size = 16.0 * current_zoom;
                if tile_size >= 4.0 {
                    let mut py = atlas_rect.min.y;
                    let mut row_idx = 0;
                    while py < atlas_rect.max.y {
                        let mut px = atlas_rect.min.x;
                        let mut col_idx = 0;
                        let row_h = tile_size.min(atlas_rect.max.y - py);
                        while px < atlas_rect.max.x {
                            let col_w = tile_size.min(atlas_rect.max.x - px);
                            let color = if (row_idx + col_idx) % 2 == 0 {
                                egui::Color32::from_rgb(26, 32, 42)
                            } else {
                                egui::Color32::from_rgb(34, 42, 54)
                            };
                            painter.rect_filled(
                                egui::Rect::from_min_size(egui::pos2(px, py), egui::vec2(col_w, row_h)),
                                0.0,
                                color,
                            );
                            px += tile_size;
                            col_idx += 1;
                        }
                        py += tile_size;
                        row_idx += 1;
                    }
                } else {
                    painter.rect_filled(atlas_rect, 0.0, egui::Color32::from_rgb(26, 32, 42));
                }

                // 2. Draw Loaded Texture image if present
                if let Some(ref tex) = self.texture_handle {
                    painter.image(
                        tex.id(),
                        atlas_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                // 3. Draw Grid Lines on texture
                if self.snap_grid && self.grid_size >= 4 {
                    let gs_world = self.grid_size as f32;
                    let gs_screen = gs_world * current_zoom;
                    if gs_screen >= 6.0 {
                        let grid_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18);
                        let mut x = 0.0;
                        while x <= ref_w {
                            let sx = to_screen(x, 0.0).x;
                            painter.line_segment([egui::pos2(sx, atlas_rect.min.y), egui::pos2(sx, atlas_rect.max.y)], egui::Stroke::new(1.0_f32, grid_color));
                            x += gs_world;
                        }
                        let mut y = 0.0;
                        while y <= ref_h {
                            let sy = to_screen(0.0, y).y;
                            painter.line_segment([egui::pos2(atlas_rect.min.x, sy), egui::pos2(atlas_rect.max.x, sy)], egui::Stroke::new(1.0_f32, grid_color));
                            y += gs_world;
                        }
                    }
                }

                // Atlas Border
                painter.rect_stroke(atlas_rect, 0.0, egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(56, 139, 253)));
                painter.text(
                    atlas_rect.min + egui::vec2(6.0, -16.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("RefSize: {} x {} px", self.imageset.ref_size.0, self.imageset.ref_size.1),
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_rgb(139, 148, 158),
                );

                // --- 4. Interactive Handling: Hit-Testing, Dragging, Resizing, Freehand Drawing ---
                let mouse_pos = ctx.input().pointer.hover_pos();
                let is_mouse_down = ctx.input().pointer.primary_down();
                let mouse_clicked = ctx.input().pointer.primary_clicked();

                let handle_size = (8.0 * current_zoom).clamp(8.0, 14.0);

                // A. Check Click to Select or Start Action
                if response.hovered() && mouse_clicked && self.tool_mode == ToolMode::DrawEdit {
                    if let Some(mp) = mouse_pos {
                        let (wx, wy) = to_world(mp);
                        let mut clicked_something = false;

                        // Check Corner Handles of Selected Box first
                        if let Some(sel_idx) = self.selected_index {
                            if sel_idx < self.imageset.images.len() {
                                let (sp_x, sp_y, sp_w, sp_h) = {
                                    let sp = &self.imageset.images[sel_idx];
                                    (sp.pos.0, sp.pos.1, sp.size.0, sp.size.1)
                                };
                                let sp_min = to_screen(sp_x as f32, sp_y as f32);
                                let sp_max = to_screen((sp_x + sp_w as i32) as f32, (sp_y + sp_h as i32) as f32);

                                let handles = [
                                    (ResizeCorner::TopLeft, egui::Rect::from_center_size(sp_min, egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::TopRight, egui::Rect::from_center_size(egui::pos2(sp_max.x, sp_min.y), egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::BottomLeft, egui::Rect::from_center_size(egui::pos2(sp_min.x, sp_max.y), egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::BottomRight, egui::Rect::from_center_size(sp_max, egui::vec2(handle_size, handle_size))),
                                ];

                                for (corner, rect) in handles {
                                    if rect.contains(mp) {
                                        self.save_undo_state();
                                        self.drag_resizing_index = Some(sel_idx);
                                        self.drag_resize_corner = Some(corner);
                                        self.drag_orig_box = Some((sp_x, sp_y, sp_w, sp_h));
                                        clicked_something = true;
                                        break;
                                    }
                                }
                            }
                        }

                        // Check clicking inside an existing box (reverse order for top-most)
                        if !clicked_something {
                            let mut hit_idx = None;
                            for (idx, sp) in self.imageset.images.iter().enumerate().rev() {
                                let bx = sp.pos.0 as f32;
                                let by = sp.pos.1 as f32;
                                let bw = sp.size.0 as f32;
                                let bh = sp.size.1 as f32;

                                if wx >= bx && wx <= bx + bw && wy >= by && wy <= by + bh {
                                    hit_idx = Some((idx, bx, by));
                                    break;
                                }
                            }

                            if let Some((idx, bx, by)) = hit_idx {
                                let modifiers = ctx.input().modifiers;
                                if modifiers.ctrl || modifiers.shift {
                                    if self.selected_indices.contains(&idx) {
                                        self.selected_indices.remove(&idx);
                                    } else {
                                        self.selected_indices.insert(idx);
                                    }
                                    self.selected_index = Some(idx);
                                } else {
                                    if !self.selected_indices.contains(&idx) {
                                        self.selected_indices.clear();
                                        self.selected_indices.insert(idx);
                                    }
                                    self.selected_index = Some(idx);
                                }

                                self.save_undo_state();
                                self.drag_moving_index = Some(idx);
                                self.drag_moving_offset = egui::vec2(wx - bx, wy - by);
                                self.drag_moving_orig_positions = self.selected_indices
                                    .iter()
                                    .copied()
                                    .filter(|&i| i < self.imageset.images.len())
                                    .map(|i| (i, self.imageset.images[i].pos))
                                    .collect();
                                if self.drag_moving_orig_positions.is_empty() {
                                    self.drag_moving_orig_positions.push((idx, self.imageset.images[idx].pos));
                                }
                                clicked_something = true;
                            }
                        }

                        // If clicked on empty canvas -> Start Freehand Box Drawing!
                        if !clicked_something && atlas_rect.contains(mp) {
                            let sn_x = self.snap(wx);
                            let sn_y = self.snap(wy);
                            self.is_drawing = true;
                            self.draw_start = Some(egui::pos2(sn_x, sn_y));
                            self.selected_index = None;
                            self.selected_indices.clear();
                        }
                    }
                }

                // B. Handle Active Drag Operations
                if is_mouse_down && self.tool_mode == ToolMode::DrawEdit {
                    if let Some(mp) = mouse_pos {
                        let (wx, wy) = to_world(mp);

                        // 1. Resizing Selected Box
                        if let (Some(r_idx), Some(corner), Some((ox, oy, ow, oh))) = (
                            self.drag_resizing_index,
                            self.drag_resize_corner,
                            self.drag_orig_box,
                        ) {
                            let cur_x = self.snap(wx) as i32;
                            let cur_y = self.snap(wy) as i32;
                            if r_idx < self.imageset.images.len() {
                                let sp = &mut self.imageset.images[r_idx];

                                match corner {
                                    ResizeCorner::BottomRight => {
                                        let new_w = (cur_x - ox).max(4);
                                        let new_h = (cur_y - oy).max(4);
                                        sp.size = (new_w as u32, new_h as u32);
                                    }
                                    ResizeCorner::TopLeft => {
                                        let right = ox + ow as i32;
                                        let bottom = oy + oh as i32;
                                        let new_x = cur_x.min(right - 4);
                                        let new_y = cur_y.min(bottom - 4);
                                        sp.pos = (new_x, new_y);
                                        sp.size = ((right - new_x) as u32, (bottom - new_y) as u32);
                                    }
                                    ResizeCorner::TopRight => {
                                        let bottom = oy + oh as i32;
                                        let new_w = (cur_x - ox).max(4);
                                        let new_y = cur_y.min(bottom - 4);
                                        sp.pos.1 = new_y;
                                        sp.size = (new_w as u32, (bottom - new_y) as u32);
                                    }
                                    ResizeCorner::BottomLeft => {
                                        let right = ox + ow as i32;
                                        let new_x = cur_x.min(right - 4);
                                        let new_h = (cur_y - oy).max(4);
                                        sp.pos.0 = new_x;
                                        sp.size = ((right - new_x) as u32, new_h as u32);
                                    }
                                }
                                self.update_code();
                            }
                        }
                        // 2. Moving Box (รองรับทั้งกล่องเดี่ยวและ Multi-selection พร้อมกัน)
                        else if let Some(m_idx) = self.drag_moving_index {
                            let target_x = self.snap(wx - self.drag_moving_offset.x) as i32;
                            let target_y = self.snap(wy - self.drag_moving_offset.y) as i32;
                            if let Some(&(_, orig_primary)) = self.drag_moving_orig_positions.iter().find(|&&(i, _)| i == m_idx) {
                                let dx = target_x - orig_primary.0;
                                let dy = target_y - orig_primary.1;
                                for &(i, orig_pos) in &self.drag_moving_orig_positions {
                                    if i < self.imageset.images.len() {
                                        self.imageset.images[i].pos = ((orig_pos.0 + dx).max(0), (orig_pos.1 + dy).max(0));
                                    }
                                }
                                self.update_code();
                            } else if m_idx < self.imageset.images.len() {
                                let sp = &mut self.imageset.images[m_idx];
                                sp.pos = (target_x.max(0), target_y.max(0));
                                self.update_code();
                            }
                        }
                        // 3. Freehand Box Drawing Preview
                        else if self.is_drawing {
                            if let Some(start) = self.draw_start {
                                let cur_x = self.snap(wx);
                                let cur_y = self.snap(wy);
                                let min_x = start.x.min(cur_x);
                                let min_y = start.y.min(cur_y);
                                let max_x = start.x.max(cur_x);
                                let max_y = start.y.max(cur_y);

                                let p_min = to_screen(min_x, min_y);
                                let p_max = to_screen(max_x, max_y);
                                let preview_rect = egui::Rect::from_min_max(p_min, p_max);

                                painter.rect_filled(
                                    preview_rect,
                                    2.0,
                                    egui::Color32::from_rgba_unmultiplied(245, 158, 11, 40),
                                );
                                painter.rect_stroke(
                                    preview_rect,
                                    2.0,
                                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(245, 158, 11)),
                                );

                                // Size tooltip while dragging
                                let drag_w = (max_x - min_x).round() as u32;
                                let drag_h = (max_y - min_y).round() as u32;
                                painter.text(
                                    p_min + egui::vec2(4.0, -16.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("{} x {} px", drag_w, drag_h),
                                    egui::FontId::monospace(12.0),
                                    egui::Color32::from_rgb(245, 158, 11),
                                );
                            }
                        }
                    }
                } else {
                    // C. Mouse Released: Finalize Box Creation or End Dragging
                    if self.is_drawing {
                        if let (Some(start), Some(mp)) = (self.draw_start, mouse_pos) {
                            let (wx, wy) = to_world(mp);
                            let cur_x = self.snap(wx);
                            let cur_y = self.snap(wy);
                            let min_x = start.x.min(cur_x).max(0.0);
                            let min_y = start.y.min(cur_y).max(0.0);
                            let max_x = start.x.max(cur_x);
                            let max_y = start.y.max(cur_y);

                            let w = (max_x - min_x).round() as u32;
                            let h = (max_y - min_y).round() as u32;

                            // Create box if user dragged at least 4x4 px
                            if w >= 4 && h >= 4 {
                                self.save_undo_state();
                                let next_num = self.imageset.images.len() + 1;
                                self.imageset.images.push(ImageDef {
                                    id: format!("icon_{}", next_num),
                                    name: format!("icon_{}", next_num),
                                    pos: (min_x as i32, min_y as i32),
                                    size: (w, h),
                                    flags: "0".to_string(),
                                    group: None,
                                });
                                let new_idx = self.imageset.images.len() - 1;
                                self.selected_index = Some(new_idx);
                                self.selected_indices.clear();
                                self.selected_indices.insert(new_idx);
                                self.status_msg = format!("Added new sprite: icon_{} ({}x{} px)", next_num, w, h);
                                self.update_code();
                            }
                        }
                        self.is_drawing = false;
                        self.draw_start = None;
                    }

                    self.drag_moving_index = None;
                    self.drag_moving_orig_positions.clear();
                    self.drag_resizing_index = None;
                    self.drag_resize_corner = None;
                    self.drag_orig_box = None;
                }

                // 5. Draw Existing Sprite Boxes & Corner Handles
                for (idx, sp) in self.imageset.images.iter().enumerate() {
                    let is_primary_sel = self.selected_index == Some(idx);
                    let is_sel = is_primary_sel || self.selected_indices.contains(&idx);
                    let sp_min = to_screen(sp.pos.0 as f32, sp.pos.1 as f32);
                    let sp_max = to_screen((sp.pos.0 + sp.size.0 as i32) as f32, (sp.pos.1 + sp.size.1 as i32) as f32);
                    let rect = egui::Rect::from_min_max(sp_min, sp_max);

                    // Box Fill & Stroke
                    let fill_color = if is_primary_sel {
                        egui::Color32::from_rgba_unmultiplied(245, 158, 11, 55)
                    } else if is_sel {
                        egui::Color32::from_rgba_unmultiplied(59, 130, 246, 50)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(59, 130, 246, 20)
                    };

                    let stroke_color = if is_primary_sel {
                        egui::Color32::from_rgb(245, 158, 11)
                    } else if is_sel {
                        egui::Color32::from_rgb(96, 165, 250)
                    } else {
                        egui::Color32::from_rgb(59, 130, 246)
                    };

                    painter.rect_filled(rect, 2.0, fill_color);
                    painter.rect_stroke(rect, 2.0, egui::Stroke::new(if is_sel { 2.0_f32 } else { 1.2_f32 }, stroke_color));

                    // Sprite Name Label Tag (แสดงเมื่อเปิด Checkbox หรือเมื่อกล่องนั้นถูกเลือกอยู่)
                    if self.show_labels || is_sel {
                        let label_text = format!("{}. {}", idx + 1, sp.name);
                        let font_id = egui::FontId::proportional(11.0);
                        painter.text(
                            rect.min + egui::vec2(4.0, 4.0),
                            egui::Align2::LEFT_TOP,
                            label_text,
                            font_id,
                            if is_sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(200, 220, 250) },
                        );
                    }

                    // Draw 4 Corner Handles for Primary Selected Box
                    if is_primary_sel {
                        let handle_fill = egui::Color32::WHITE;
                        let handle_stroke = egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(245, 158, 11));

                        let corner_points = [
                            sp_min,
                            egui::pos2(sp_max.x, sp_min.y),
                            egui::pos2(sp_min.x, sp_max.y),
                            sp_max,
                        ];

                        for pt in corner_points {
                            let h_rect = egui::Rect::from_center_size(pt, egui::vec2(handle_size, handle_size));
                            painter.rect_filled(h_rect, 1.0, handle_fill);
                            painter.rect_stroke(h_rect, 1.0, handle_stroke);
                        }
                    }
                }
            });
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Check if CLI mode was requested
    if args.len() > 1 && (args.contains(&"--parse".to_string()) || args.contains(&"--generate".to_string())) {
        let mut mode = "parse".to_string();
        let mut input_path = "".to_string();
        let mut output_path = "".to_string();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--parse" => mode = "parse".to_string(),
                "--generate" => mode = "generate".to_string(),
                "--input" => {
                    if i + 1 < args.len() { input_path = args[i + 1].clone(); i += 1; }
                }
                "--output" => {
                    if i + 1 < args.len() { output_path = args[i + 1].clone(); i += 1; }
                }
                _ => {}
            }
            i += 1;
        }

        if input_path.is_empty() {
            eprintln!("Error: --input <file> is required.");
            std::process::exit(1);
        }

        let content = fs::read_to_string(&input_path).unwrap_or_else(|e| {
            eprintln!("Error reading '{}': {}", input_path, e);
            std::process::exit(1);
        });

        let imageset = parse_imageset(&content);

        if mode == "parse" {
            println!("=== Parsed .imageset ===");
            println!("Name: {}", imageset.name);
            println!("RefSize: {} {}", imageset.ref_size.0, imageset.ref_size.1);
            println!("Texture 1x: {}", imageset.texture_path_1x);
            if imageset.enable_2x {
                println!("Texture 2x: {}", imageset.texture_path_2x);
            }
            println!("Total Sprites: {}", imageset.images.len());
            for (idx, img) in imageset.images.iter().enumerate() {
                println!(" [{}] \"{}\" - Pos: ({}, {}), Size: ({}x{}), Flags: {}", idx + 1, img.name, img.pos.0, img.pos.1, img.size.0, img.size.1, img.flags);
            }
        } else {
            let output_code = generate_imageset(&imageset);
            if !output_path.is_empty() {
                fs::write(&output_path, &output_code).unwrap();
                println!("Successfully saved formatted imageset to '{}'", output_path);
            } else {
                print!("{}", output_code);
            }
        }
        return;
    }

    // Default: Run as Native Desktop GUI App
    let mut icon_data = None;
    if let Ok(img) = image::load_from_memory(include_bytes!("../app_icon.png")) {
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        icon_data = Some(eframe::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }

    let native_options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1656.0, 1067.0)),
        min_window_size: Some(egui::vec2(1200.0, 720.0)),
        maximized: false,
        icon_data,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "ImageSet Editor",
        native_options,
        Box::new(|_cc| Box::new(ImagesetEditorApp::default())),
    );
}
