# DayZ ImageSets Visual Editor

<p align="center">
  <img src="app_icon.png" alt="DayZ ImageSets Editor Logo" width="128" height="128">
</p>

<p align="center">
  <strong>A modern, standalone visual editor for DayZ Enfusion <code>.imageset</code> files and textures (<code>.edds</code> / <code>.png</code>).</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#how-to-use">How to Use</a> •
  <a href="#building-from-source">Building from Source</a> •
  <a href="#dayz-integration">DayZ Integration</a> •
  <a href="#license">License</a>
</p>

---

## 🌟 Overview (ภาพรวม)

In DayZ modding, defining custom UI icons and sprites via Bohemia Interactive's Workbench ImageSet Editor can be tedious, restrictive, or prone to formatting issues. 

**DayZ ImageSets Visual Editor** is built with Rust and `eframe/egui` to provide a blazing-fast, lightweight, and modern alternative for DayZ modders. It allows you to visually inspect textures, drag & draw bounding boxes for sprites, and export standard Enfusion-compatible `.imageset` files with accurate DayZ `{GUID}` preservation.

---

## ✨ Features (คุณสมบัติเด่น)

- 🎨 **Direct `.edds` Texture Support & 1-Click Export:**
  - Automatically decompresses and loads DayZ `.edds` textures using embedded `edds2png` support.
  - **1-Click `📦 Export EDDS + .meta`:** Converts textures to `.edds` and generates `.edds.meta` with registered `{GUID}` automatically.
- 🪄 **Smart Auto-Detection & Auto-Trim (New in v0.2.0):**
  - **`🔍 Auto-Detect`:** Scans the entire texture sheet for non-transparent pixel islands (Connected-Component Alpha Detection) and generates sprite boxes for all icons in 1 click.
  - **`🪄 Auto-Trim`:** Automatically shrinks/fits selected bounding boxes tightly around non-transparent icon pixels (`Alpha > 10`).
- 📐 **Visual Bounding Box Manipulation:**
  - Drag to move single or multiple sprite boxes (`Ctrl+Click` / `Ctrl+A`) simultaneously.
  - Interactive corner handles to resize width and height with real-time feedback.
  - **Arrow Keys Nudge:** Move selected boxes by `1px` (`Arrow Keys`) or by grid size (`Shift + Arrow Keys`).
  - Configurable **Snap to Grid** (default 8px) with toggleable grid sizes.
  - **Undo / Redo (`Ctrl + Z` / `Ctrl + Y`):** Up to 50 steps of history tracking.
- 🔍 **DayZ GUID & Meta Resolution:**
  - Preserves `{GUID}` paths (e.g. `{534691EE0479871C}Gui/imagesets/dayz_gui.edds`).
  - Automatically scans for corresponding `.edds.meta` files to discover and attach the correct GUID if missing.
- 📋 **Sprite Management & Search:**
  - **Search & Filter (`🔍 Search...`):** Quickly filter sprites by name or group, and click **`🎯`** (or double-click a card) to focus the canvas on that sprite.
  - **Copy / Paste / Duplicate (`Ctrl + C` / `Ctrl + V` / `Ctrl + D`):** Quick duplication and clipboard support.
  - **Copy Enforce Path (`📋 Copy set:...`):** One-click copy of `set:<imageset> image:<sprite>` for Enforce Script & `.layout` files.
  - **Drag & Drop Support:** Drop `.edds`, `.png`, `.tga`, `.jpg`, or `.imageset` files directly into the window.
- 💻 **Live Enfusion Code Generation:** Instant deterministic preview of the generated DayZ `ImageSetClass` hierarchy with one-click **Copy Code**.
- 🚀 **Zero External Dependencies:** Single portable native desktop executable (with `edds2png` and `imagesetconv` embedded inside).

---

## 📥 Installation (การติดตั้งและการดาวน์โหลด)

1. Download the latest release from the [GitHub Releases](https://github.com/GreenDevth/DayZ-ImageSet-Editor/releases) page.
2. Extract the archive into any folder on your computer.
3. Ensure `edds2png.exe` is placed alongside `dayz_imageset_editor.exe` (included by default).
4. Run `dayz_imageset_editor.exe` directly!

---

## 🛠️ How to Use (คู่มือการใช้งานเบื้องต้น)

### 1. Loading Textures
- Click **`Load Texture (EDDS/PNG)`** to open your mod's texture sheet (`.edds`, `.png`, `.tga`, `.jpg`).
- The editor will automatically detect resolution (RefSize) and display it on the interactive pan/zoom canvas.

### 2. Importing Existing `.imageset`
- Click **`Import .imageset`** to open any DayZ imageset file (e.g. `dayz_gui.imageset` or custom mod files).
- The editor parses all `ImageSetDefClass` and `ImageSetGroupClass` items and locates the corresponding texture.

### 3. Canvas Controls & Hotkeys
| Action | Shortcut / Control |
|---|---|
| **Pan Canvas** | Right / Middle Mouse Drag (or switch to `Pan Mode`) |
| **Zoom Canvas** | Mouse Scroll Wheel (or `+` / `-` buttons) |
| **Reset View** | `Fit / Reset View` button |
| **Focus on Sprite** | Click `🎯` or Double-Click sprite card in left panel |
| **Draw New Box** | Left Mouse Drag on canvas (in `Draw/Edit Mode`) |
| **Move Box(es)** | Left Mouse Drag inside box (or `Arrow Keys` for 1px / `Shift + Arrows` for Grid) |
| **Resize Box** | Left Mouse Drag any of the 4 corner handles |
| **Auto-Trim Box** | Click `🪄 Auto-Trim` to fit tightly around icon pixels |
| **Auto-Detect All** | Click `🔍 Auto-Detect` to generate boxes for all icons on sheet |
| **Multi-Select** | `Ctrl + Click` or `Ctrl + A` (Select All) |
| **Undo / Redo** | `Ctrl + Z` / `Ctrl + Y` (or `Ctrl + Shift + Z`) |
| **Copy / Paste / Duplicate** | `Ctrl + C` / `Ctrl + V` / `Ctrl + D` |
| **Delete Selected** | `Delete` or `Backspace` (when not editing text) |

### 4. Saving
- Click **`Save .imageset`** to output the clean, Enfusion-formatted `.imageset` file ready for packaging into your DayZ mod PBO.

---

## 🧱 Building from Source (การคอมไพล์จากโค้ด)

### Prerequisites
- [Rust & Cargo](https://rustup.rs/) (1.70+ recommended)
- Visual Studio C++ Build Tools (Windows)

### Build Commands
```bash
# Clone the repository
git clone https://github.com/GreenDevth/DayZ-ImageSet-Editor.git
cd DayZ-ImageSet-Editor

# Build release executable
cargo build --release
```
The compiled executable will be located at:
```
target/release/dayz_imageset_editor.exe
```

---

## 🎮 DayZ Integration Example (ตัวอย่างการนำไปใช้ใน Mod)

Once you save your `.imageset` (e.g., `vehicles_icons.imageset`), register it in your mod's `config.cpp`:

```cpp
class CfgPatches
{
    class MyMod_GUI
    {
        units[] = {};
        weapons[] = {};
        requiredVersion = 0.1;
        requiredAddons[] = {"DZ_Data", "DZ_GUI"};
    };
};

class CfgVehicles
{
    // Mod definitions...
};
```

And in your Enforce Script (`.c`):
```c
// โหลดไอคอนจาก ImageSet เข้าสู่ ImageWidget
ImageWidget iconWidget = ImageWidget.Cast(layoutRoot.FindAnyWidget("MyIcon"));
if (iconWidget)
{
    iconWidget.LoadImageFile(0, "set:vehicles_icons image:remote_battery");
}
```

---

## 📄 License
This project is licensed under the [MIT License](LICENSE).

---

<p align="center">
  Developed with ❤️ for the DayZ Modding Community by <strong>GreenDevth</strong>.
</p>
