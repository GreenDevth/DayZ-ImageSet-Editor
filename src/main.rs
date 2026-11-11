use eframe::egui;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    // Groups block
    let mut groups: HashMap<String, Vec<&ImageDef>> = HashMap::new();
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
            status_msg: "Ready: Click & drag on canvas to draw sprite boxes, or load texture".to_string(),
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
    fn new_project(&mut self) {
        self.imageset = ImageSet {
            name: "MyMod_Icons".to_string(),
            ref_size: (1024, 1024),
            texture_path_1x: "Gui/imagesets/MyMod_Icons.edds".to_string(),
            texture_path_2x: "Gui/imagesets/MyMod_Icons@2x.edds".to_string(),
            enable_2x: false,
            images: Vec::new(),
        };
        self.selected_index = None;
        self.texture_handle = None;
        self.loaded_texture_path = None;
        self.loaded_rgba_image = None;
        self.is_drawing = false;
        self.draw_start = None;
        self.drag_moving_index = None;
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

    // ฟังก์ชันค้นหาและแปลง .edds ไปเป็น .png โดยใช้ edds2png.exe
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

        // ค้นหาพาธของ edds2png.exe ในโฟลเดอร์โปรแกรมหรือโฟลเดอร์เครื่องมือ
        let exe_candidates = [
            PathBuf::from("edds2png.exe"),
            PathBuf::from("tools/edds2png.exe"),
            PathBuf::from("../tools/edds2png.exe"),
            PathBuf::from("P:/workbrench dayz tools/tools/edds2png.exe"),
            PathBuf::from("P:/workbrench dayz tools/edds2png.exe"),
        ];

        let mut tool_exe = None;
        for c in &exe_candidates {
            if c.exists() {
                tool_exe = Some(c.clone());
                break;
            }
        }

        if let Some(tool) = tool_exe {
            let _ = Command::new(&tool)
                .arg(edds_path)
                .output();

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
                self.selected_index = Some(self.imageset.images.len() - 1);
                self.status_msg = format!("Duplicated sprite: {}", self.imageset.images.last().unwrap().name);
                self.update_code();
            }
        }
    }

    // เมธอดลบ (Delete) กล่อง Sprite ที่เลือก
    fn delete_selected_box(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx < self.imageset.images.len() {
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

        // 2. ค้นหา imagesetconv.exe
        let conv_candidates = [
            PathBuf::from("imagesetconv.exe"),
            PathBuf::from("tools/imagesetconv.exe"),
            PathBuf::from("../imagesetconv.exe"),
            PathBuf::from("P:/workbrench dayz tools/tools/imagesetconv.exe"),
            PathBuf::from("P:/workbrench dayz tools/imagesetconv.exe"),
            PathBuf::from("P:/workbrench dayz tools/DayZ-ImageSet-Editor/imagesetconv.exe"),
        ];

        let mut conv_exe = None;
        for c in &conv_candidates {
            if c.exists() {
                conv_exe = Some(c.clone());
                break;
            }
        }

        let conv_tool = conv_exe.ok_or_else(|| "imagesetconv.exe not found! Please place it in the application folder.".to_string())?;

        // 3. เรียกคำสั่ง png2edds
        let output = Command::new(&conv_tool)
            .args(&["png2edds", "-png", &png_to_convert.to_string_lossy(), "-output", &output_edds_path.to_string_lossy()])
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

        // จัดการคีย์ลัดสำหรับ Duplicate (Ctrl+D) และ Delete (Delete / Backspace เมื่อไม่ได้พิมพ์ข้อความ)
        if ctx.input().modifiers.ctrl && ctx.input().key_pressed(egui::Key::D) {
            self.duplicate_selected_box();
        }
        if !ctx.wants_keyboard_input() && (ctx.input().key_pressed(egui::Key::Delete) || ctx.input().key_pressed(egui::Key::Backspace)) {
            self.delete_selected_box();
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
                            if let Ok(content) = fs::read_to_string(&path) {
                                self.imageset = parse_imageset(&content);
                                self.selected_index = if !self.imageset.images.is_empty() { Some(0) } else { None };
                                self.status_msg = format!("Imported: {} sprite(s)", self.imageset.images.len());
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

                    // ด้านขวาสุดแสดง Version ของ Application และสถานะ
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("v0.1.0")
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
            .default_width(320.0)
            .width_range(280.0..=480.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 24, 32))
                    .inner_margin(egui::style::Margin::same(14.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 56, 70)))
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("1. ImageSet Properties")
                        .strong()
                        .size(13.0)
                        .color(egui::Color32::from_rgb(139, 148, 158))
                );
                ui.add_space(8.0);

                let mut changed = false;

                // Name field
                ui.label(egui::RichText::new("ImageSet Name:").size(12.0));
                if ui.add(egui::TextEdit::singleline(&mut self.imageset.name).desired_width(f32::INFINITY)).changed() {
                    changed = true;
                }
                ui.add_space(6.0);

                // Texture path mpix 0
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Texture (mpix 0) .edds:").size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("(Ctrl+V to paste GUID)").size(10.0).color(egui::Color32::from_rgb(139, 148, 158)));
                    });
                });

                if ui.add(
                    egui::TextEdit::singleline(&mut self.imageset.texture_path_1x)
                        .desired_width(f32::INFINITY)
                        .hint_text("{GUID}Gui/imagesets/name.edds")
                ).changed() {
                    changed = true;
                }
                ui.add_space(6.0);

                // Optional mpix 1 (@2x) texture
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.imageset.enable_2x, "Enable 2x Texture (mpix 1):").changed() {
                        changed = true;
                    }
                });

                if self.imageset.enable_2x {
                    if ui.add(
                        egui::TextEdit::singleline(&mut self.imageset.texture_path_2x)
                            .desired_width(f32::INFINITY)
                            .hint_text("{GUID}Gui/imagesets/name@2x.edds")
                    ).changed() {
                        changed = true;
                    }
                }
                ui.add_space(6.0);

                // RefSize W x H
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Ref Width:").size(11.0));
                        let mut w_str = self.imageset.ref_size.0.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut w_str).desired_width(110.0)).changed() {
                            if let Ok(w) = w_str.parse::<u32>() { self.imageset.ref_size.0 = w; changed = true; }
                        }
                    });

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Ref Height:").size(11.0));
                        let mut h_str = self.imageset.ref_size.1.to_string();
                        if ui.add(egui::TextEdit::singleline(&mut h_str).desired_width(110.0)).changed() {
                            if let Ok(h) = h_str.parse::<u32>() { self.imageset.ref_size.1 = h; changed = true; }
                        }
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                // Section 2: Sprites List Header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("2. Sprites ({})", self.imageset.images.len()))
                            .strong()
                            .size(13.0)
                            .color(egui::Color32::from_rgb(139, 148, 158))
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add Box").on_hover_text("เพิ่มกล่อง Sprite ใหม่ที่มุมบนซ้าย").clicked() {
                            let next_num = self.imageset.images.len() + 1;
                            self.imageset.images.push(ImageDef {
                                id: format!("icon_{}", next_num),
                                name: format!("icon_{}", next_num),
                                pos: (16, 16),
                                size: (64, 64),
                                flags: "0".to_string(),
                                group: None,
                            });
                            self.selected_index = Some(self.imageset.images.len() - 1);
                            changed = true;
                        }

                        // ปุ่ม Duplicate Box ใน Header
                        let has_selection = self.selected_index.is_some();
                        let dup_btn = egui::Button::new("📋 Duplicate");
                        if ui.add_enabled(has_selection, dup_btn).on_hover_text("คัดลอกกล่อง Sprite ที่เลือกอยู่ (Ctrl+D)").clicked() {
                            self.duplicate_selected_box();
                            changed = true;
                        }
                    });
                });
                ui.add_space(6.0);

                // Sprites List Body
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.imageset.images.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("No sprite boxes yet.\nDrag on canvas to draw freely, or click '+ Add Box'")
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(100, 115, 130))
                            );
                        });
                    }

                    let mut to_delete = None;
                    for (idx, img) in self.imageset.images.iter().enumerate() {
                        let is_sel = self.selected_index == Some(idx);
                        let bg_color = if is_sel {
                            egui::Color32::from_rgb(38, 48, 62)
                        } else {
                            egui::Color32::from_rgb(26, 32, 40)
                        };

                        let mut del_clicked = false;
                        let item_frame = egui::Frame::none()
                            .fill(bg_color)
                            .inner_margin(egui::style::Margin::symmetric(10.0, 8.0))
                            .stroke(egui::Stroke::new(1.0_f32, if is_sel { egui::Color32::from_rgb(245, 158, 11) } else { egui::Color32::from_rgb(38, 46, 56) }));

                        let frame_resp = item_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let text = format!("{}. {}", idx + 1, img.name);
                                let subtext = format!("Pos: {}, {} | Size: {}x{}", img.pos.0, img.pos.1, img.size.0, img.size.1);
                                
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(text).strong().size(12.0).color(if is_sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(210, 225, 240) }));
                                    ui.label(egui::RichText::new(subtext).size(10.0).color(egui::Color32::from_rgb(120, 135, 150)));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let del_btn = egui::Button::new(
                                        egui::RichText::new("✕").size(12.0).color(egui::Color32::from_rgb(248, 113, 113))
                                    );
                                    if ui.add(del_btn).on_hover_text("Delete this sprite box").clicked() {
                                        del_clicked = true;
                                    }
                                });
                            });
                        });

                        if del_clicked {
                            to_delete = Some(idx);
                        } else if ui.interact(frame_resp.response.rect, egui::Id::new(("sprite_card", idx)), egui::Sense::click()).clicked() {
                            // คลิกที่ส่วนใดก็ได้ของการ์ดกล่องเพื่อ Active (Select) เมื่อไม่ได้กดลบ
                            self.selected_index = Some(idx);
                        }

                        ui.add_space(4.0);
                    }

                    if let Some(idx) = to_delete {
                        self.imageset.images.remove(idx);
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
            .default_width(360.0)
            .width_range(300.0..=550.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 24, 32))
                    .inner_margin(egui::style::Margin::same(14.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 56, 70)))
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("3. Sprite Inspector")
                        .strong()
                        .size(13.0)
                        .color(egui::Color32::from_rgb(139, 148, 158))
                );
                ui.add_space(8.0);

                let mut changed = false;

                if let Some(idx) = self.selected_index {
                    if idx < self.imageset.images.len() {
                        let sp = &mut self.imageset.images[idx];

                        ui.label(egui::RichText::new("Sprite Name:").size(12.0));
                        if ui.add(egui::TextEdit::singleline(&mut sp.name).desired_width(f32::INFINITY)).changed() {
                            changed = true;
                        }
                        ui.add_space(6.0);

                        // Coordinates in 2 columns
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Pos X (Left):").size(11.0));
                                let mut x = sp.pos.0.to_string();
                                if ui.add(egui::TextEdit::singleline(&mut x).desired_width(95.0)).changed() {
                                    if let Ok(val) = x.parse::<i32>() { sp.pos.0 = val; changed = true; }
                                }
                            });

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Pos Y (Top):").size(11.0));
                                let mut y = sp.pos.1.to_string();
                                if ui.add(egui::TextEdit::singleline(&mut y).desired_width(95.0)).changed() {
                                    if let Ok(val) = y.parse::<i32>() { sp.pos.1 = val; changed = true; }
                                }
                            });
                        });
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Size W (Width):").size(11.0));
                                let mut w = sp.size.0.to_string();
                                if ui.add(egui::TextEdit::singleline(&mut w).desired_width(95.0)).changed() {
                                    if let Ok(val) = w.parse::<u32>() { sp.size.0 = val; changed = true; }
                                }
                            });

                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Size H (Height):").size(11.0));
                                let mut h = sp.size.1.to_string();
                                if ui.add(egui::TextEdit::singleline(&mut h).desired_width(95.0)).changed() {
                                    if let Ok(val) = h.parse::<u32>() { sp.size.1 = val; changed = true; }
                                }
                            });
                        });
                        ui.add_space(6.0);

                        // Flags field with presets (0, ISVerticalTile, ISHorizontalTile, 3)
                        ui.label(egui::RichText::new("Flags (Enum / Number):").size(11.0));
                        ui.horizontal(|ui| {
                            if ui.add(egui::TextEdit::singleline(&mut sp.flags).desired_width(120.0)).changed() {
                                changed = true;
                            }

                            // Quick preset pills
                            if ui.small_button("0").on_hover_text("Standard 0").clicked() {
                                sp.flags = "0".to_string();
                                changed = true;
                            }
                            if ui.small_button("VTile").on_hover_text("ISVerticalTile").clicked() {
                                sp.flags = "ISVerticalTile".to_string();
                                changed = true;
                            }
                            if ui.small_button("HTile").on_hover_text("ISHorizontalTile").clicked() {
                                sp.flags = "ISHorizontalTile".to_string();
                                changed = true;
                            }
                            if ui.small_button("3").on_hover_text("Grid Center 3").clicked() {
                                sp.flags = "3".to_string();
                                changed = true;
                            }
                        });
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Group (Optional):").size(11.0));
                                let mut grp = sp.group.clone().unwrap_or_default();
                                if ui.add(egui::TextEdit::singleline(&mut grp).desired_width(f32::INFINITY)).changed() {
                                    sp.group = if grp.trim().is_empty() { None } else { Some(grp) };
                                    changed = true;
                                }
                            });
                        });

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            // ปุ่ม Duplicate Box ใน Inspector
                            let dup_btn = egui::Button::new(
                                egui::RichText::new("📋 Duplicate (Ctrl+D)").size(11.0)
                            ).fill(egui::Color32::from_rgb(37, 99, 235));

                            if ui.add(dup_btn).on_hover_text("ทำสำเนากล่องนี้พร้อมขนาดและค่าต่างๆ ไปยังตำแหน่งถัดไป").clicked() {
                                self.duplicate_selected_box();
                                changed = true;
                            }

                            // ปุ่ม Delete Box ใน Inspector
                            let del_btn = egui::Button::new(
                                egui::RichText::new("🗑 Delete Box (Del)").size(11.0)
                            ).fill(egui::Color32::from_rgb(185, 28, 28));

                            if ui.add(del_btn).on_hover_text("ลบกล่อง Sprite นี้ (กดปุ่ม Delete หรือ Backspace)").clicked() {
                                self.delete_selected_box();
                                changed = true;
                            }
                        });
                    }
                } else {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(16, 20, 26))
                        .inner_margin(egui::style::Margin::same(12.0))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(34, 42, 54)))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("No sprite selected.\nClick any box on canvas to inspect/edit.")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(120, 135, 150))
                            );
                        });
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                // Section 4: Live Code Header with Proper Padding and Clean Container
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("4. Live .imageset Code")
                            .strong()
                            .size(13.0)
                            .color(egui::Color32::from_rgb(139, 148, 158))
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copy Code").clicked() {
                            ui.output().copied_text = self.code_cache.clone();
                            self.status_msg = "Code copied to clipboard!".to_string();
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
                    .inner_margin(egui::style::Margin::symmetric(14.0, 8.0))
                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(38, 46, 58)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Tool Mode Selector (Draw/Edit vs Pan)
                            let is_draw = self.tool_mode == ToolMode::DrawEdit;
                            let is_pan = self.tool_mode == ToolMode::Pan;

                            let draw_btn = egui::Button::new("Draw / Edit Mode")
                                .fill(if is_draw { egui::Color32::from_rgb(30, 60, 90) } else { egui::Color32::from_rgb(26, 32, 40) });
                            if ui.add(draw_btn).clicked() {
                                self.tool_mode = ToolMode::DrawEdit;
                            }

                            let pan_btn = egui::Button::new("Pan Mode")
                                .fill(if is_pan { egui::Color32::from_rgb(30, 60, 90) } else { egui::Color32::from_rgb(26, 32, 40) });
                            if ui.add(pan_btn).clicked() {
                                self.tool_mode = ToolMode::Pan;
                            }

                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(16.0);

                            // Snap Grid controls
                            ui.checkbox(&mut self.snap_grid, "Snap Grid:");
                            ui.add(egui::Slider::new(&mut self.grid_size, 1..=64).suffix(" px"));

                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(16.0);

                            // Zoom controls
                            if ui.button(" - ").clicked() { self.zoom = (self.zoom / 1.15_f32).max(0.1_f32); }
                            ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).monospace());
                            if ui.button(" + ").clicked() { self.zoom = (self.zoom * 1.15_f32).min(8.0_f32); }

                            if ui.button("Fit / Reset View").clicked() {
                                self.pan = egui::vec2(60.0, 60.0);
                                self.zoom = 0.65;
                            }

                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(16.0);

                            // Toggle Display Sprite Names Checkbox
                            ui.checkbox(&mut self.show_labels, "Show Names");
                        });
                    });

                // Canvas area allocation
                let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
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
                let half_h = handle_size * 0.5;

                // A. Check Click to Select or Start Action
                if response.hovered() && mouse_clicked && self.tool_mode == ToolMode::DrawEdit {
                    if let Some(mp) = mouse_pos {
                        let (wx, wy) = to_world(mp);
                        let mut clicked_something = false;

                        // Check Corner Handles of Selected Box first
                        if let Some(sel_idx) = self.selected_index {
                            if sel_idx < self.imageset.images.len() {
                                let sp = &self.imageset.images[sel_idx];
                                let sp_min = to_screen(sp.pos.0 as f32, sp.pos.1 as f32);
                                let sp_max = to_screen((sp.pos.0 + sp.size.0 as i32) as f32, (sp.pos.1 + sp.size.1 as i32) as f32);

                                let handles = [
                                    (ResizeCorner::TopLeft, egui::Rect::from_center_size(sp_min, egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::TopRight, egui::Rect::from_center_size(egui::pos2(sp_max.x, sp_min.y), egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::BottomLeft, egui::Rect::from_center_size(egui::pos2(sp_min.x, sp_max.y), egui::vec2(handle_size, handle_size))),
                                    (ResizeCorner::BottomRight, egui::Rect::from_center_size(sp_max, egui::vec2(handle_size, handle_size))),
                                ];

                                for (corner, rect) in handles {
                                    if rect.contains(mp) {
                                        self.drag_resizing_index = Some(sel_idx);
                                        self.drag_resize_corner = Some(corner);
                                        self.drag_orig_box = Some((sp.pos.0, sp.pos.1, sp.size.0, sp.size.1));
                                        clicked_something = true;
                                        break;
                                    }
                                }
                            }
                        }

                        // Check clicking inside an existing box (reverse order for top-most)
                        if !clicked_something {
                            for (idx, sp) in self.imageset.images.iter().enumerate().rev() {
                                let bx = sp.pos.0 as f32;
                                let by = sp.pos.1 as f32;
                                let bw = sp.size.0 as f32;
                                let bh = sp.size.1 as f32;

                                if wx >= bx && wx <= bx + bw && wy >= by && wy <= by + bh {
                                    self.selected_index = Some(idx);
                                    self.drag_moving_index = Some(idx);
                                    self.drag_moving_offset = egui::vec2(wx - bx, wy - by);
                                    clicked_something = true;
                                    break;
                                }
                            }
                        }

                        // If clicked on empty canvas -> Start Freehand Box Drawing!
                        if !clicked_something && atlas_rect.contains(mp) {
                            let sn_x = self.snap(wx);
                            let sn_y = self.snap(wy);
                            self.is_drawing = true;
                            self.draw_start = Some(egui::pos2(sn_x, sn_y));
                            self.selected_index = None;
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
                        // 2. Moving Box
                        else if let Some(m_idx) = self.drag_moving_index {
                            let target_x = self.snap(wx - self.drag_moving_offset.x) as i32;
                            let target_y = self.snap(wy - self.drag_moving_offset.y) as i32;
                            if m_idx < self.imageset.images.len() {
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
                                let next_num = self.imageset.images.len() + 1;
                                self.imageset.images.push(ImageDef {
                                    id: format!("icon_{}", next_num),
                                    name: format!("icon_{}", next_num),
                                    pos: (min_x as i32, min_y as i32),
                                    size: (w, h),
                                    flags: "0".to_string(),
                                    group: None,
                                });
                                self.selected_index = Some(self.imageset.images.len() - 1);
                                self.status_msg = format!("Added new sprite: icon_{} ({}x{} px)", next_num, w, h);
                                self.update_code();
                            }
                        }
                        self.is_drawing = false;
                        self.draw_start = None;
                    }

                    self.drag_moving_index = None;
                    self.drag_resizing_index = None;
                    self.drag_resize_corner = None;
                    self.drag_orig_box = None;
                }

                // 5. Draw Existing Sprite Boxes & Corner Handles
                for (idx, sp) in self.imageset.images.iter().enumerate() {
                    let is_sel = self.selected_index == Some(idx);
                    let sp_min = to_screen(sp.pos.0 as f32, sp.pos.1 as f32);
                    let sp_max = to_screen((sp.pos.0 + sp.size.0 as i32) as f32, (sp.pos.1 + sp.size.1 as i32) as f32);
                    let rect = egui::Rect::from_min_max(sp_min, sp_max);

                    // Box Fill & Stroke
                    let fill_color = if is_sel {
                        egui::Color32::from_rgba_unmultiplied(245, 158, 11, 45)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(59, 130, 246, 25)
                    };

                    let stroke_color = if is_sel {
                        egui::Color32::from_rgb(245, 158, 11)
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

                    // Draw 4 Corner Handles for Selected Box
                    if is_sel {
                        let handle_fill = egui::Color32::WHITE;
                        let handle_stroke = egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(245, 158, 11));

                        let corner_points = [
                            sp_min,
                            egui::pos2(sp_max.x, sp_min.y),
                            egui::pos2(sp_min.x, sp_max.y),
                            sp_max,
                        ];

                        for pt in corner_points {
                            let h_rect = egui::Rect::from_min_max(
                                pt - egui::vec2(half_h, half_h),
                                pt + egui::vec2(half_h, half_h),
                            );
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
        initial_window_size: Some(egui::vec2(1440.0, 920.0)),
        min_window_size: Some(egui::vec2(1100.0, 720.0)),
        icon_data,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "ImageSet Editor",
        native_options,
        Box::new(|_cc| Box::new(ImagesetEditorApp::default())),
    );
}
