# DayZ ImageSets Visual Editor - Wiki Documentation

ยินดีต้อนรับสู่คู่มือการใช้งานและการพัฒนาของโปรแกรม **DayZ ImageSets Visual Editor**

---

## สารบัญ (Table of Contents)

1. [ภาพรวมและข้อกำหนดเบื้องต้น (Overview & Requirements)](#1-ภาพรวมและข้อกำหนดเบื้องต้น)
2. [คู่มือการใช้งานโปรแกรม (User Manual)](#2-คู่มือการใช้งานโปรแกรม)
   - [การนำเข้า Texture](#การนำเข้า-texture)
   - [การสร้างและจัดการ Sprite Bounding Boxes](#การสร้างและจัดการ-sprite-bounding-boxes)
   - [การแก้ไขขนาดและพิกัดแบบแม่นยำ](#การแก้ไขขนาดและพิกัดแบบแม่นยำ)
   - [การใช้งานระบบ GUID และ .meta](#การใช้งานระบบ-guid-และ-meta)
   - [การบันทึกและการส่งออก .imageset](#การบันทึกและการส่งออก-imageset)
3. [การนำไปใช้งานใน DayZ Modding (DayZ Enforce & Layout Integration)](#3-การนำไปใช้งานใน-dayz-modding)
   - [โครงสร้างโฟลเดอร์ Mod](#โครงสร้างโฟลเดอร์-mod)
   - [การเรียกใช้ใน .layout](#การเรียกใช้ใน-layout)
   - [ตัวอย่างโค้ด Enforce Script (.c)](#ตัวอย่างโค้ด-enforce-script-c)
4. [โครงสร้างโค้ดและการต่อยอดสำหรับนักพัฒนา (Developer Guide)](#4-โครงสร้างโค้ดและการต่อยอดสำหรับนักพัฒนา)

---

## 1. ภาพรวมและข้อกำหนดเบื้องต้น

DayZ ใช้นามสกุลไฟล์ `.imageset` (Enfusion Engine format) ในการจับคู่พิกัดภาพ (Texture Atlas) เข้ากับชื่อไอคอน เพื่อให้สามารถเรียกใช้งานในเมนู, HUD, หน้าต่างคลังเก็บของ (Inventory) หรือวิดเจ็ตในเกมได้อย่างมีประสิทธิภาพ

### ข้อกำหนดของระบบ
- **ระบบปฏิบัติการ:** Windows 10 / 11 (64-bit)
- **เครื่องมือเสริม:** `edds2png.exe` (รวมมาให้พร้อมกับโปรแกรม) วางไว้ในโฟลเดอร์เดียวกันสำหรับเปิดภาพ `.edds`

---

## 2. คู่มือการใช้งานโปรแกรม

### การนำเข้า Texture
- คลิกปุ่ม **`Load Texture (EDDS/PNG)`** ที่แถบเมนูด้านบน
- เลือกไฟล์ภาพที่ต้องการรองรับทั้ง `.edds`, `.png`, `.tga`, `.jpg`
- หากเป็นไฟล์ `.edds` ตัวโปรแกรมจะเรียก `edds2png` มาคลายการบีบอัด LZ4 Stream และแสดงผลภาพบน Canvas อัตโนมัติ

### การสร้างและจัดการ Sprite Bounding Boxes
- **วาดกล่องใหม่:** สลับโหมดเป็น `Draw/Edit Mode` จากนั้นคลิกซ้ายและลากเมาส์ครอบบริเวณไอคอนที่ต้องการบน Canvas
- **ย้ายตำแหน่งกล่อง:** คลิกค้างที่บริเวณด้านในกล่องแล้วลากเพื่อขยับตำแหน่ง
- **ปรับขนาดกล่อง:** ชี้เมาส์ที่มุมกล่องทั้ง 4 มุม (จะมีไฮไลต์สี่เหลี่ยมสีส้มขึ้น) แล้วคลิกลากเพื่อย่อ/ขยาย
- **คัดลอกกล่อง (Duplicate):** คลิกเลือกกล่องแล้วกด `Ctrl + D` หรือกดปุ่ม `📋 Duplicate` ในเมนู
- **ลบกล่อง (Delete):** คลิกเลือกกล่องแล้วกดปุ่ม `Delete` หรือ `Backspace` บนคีย์บอร์ด หรือกดปุ่ม `✕` บนการ์ด Sprite

### การแก้ไขขนาดและพิกัดแบบแม่นยำ
ในแถบด้านขวา **3. Sprite Inspector**:
- สามารถพิมพ์พิกัด `Pos X`, `Pos Y`, `Size W`, `Size H` เป็นตัวเลขที่แน่นอนได้
- กำหนดค่า `Flags`:
  - `0`: รูปแบบปกติ
  - `ISVerticalTile`: ปูกระเบื้องแนวตั้ง
  - `ISHorizontalTile`: ปูกระเบื้องแนวนอน
  - `3`: Grid Center (สำหรับขยายขอบ 9-slice)

### การใช้งานระบบ GUID และ .meta
DayZ มีระบบ Enfusion Resource Identifier ในรูป `{GUID}path.edds` ตัวโปรแกรมจะ:
1. คงค่า `{GUID}` เดิมจากไฟล์ `.imageset` เสมอ
2. หากไม่มี GUID ในไฟล์ โปรแกรมจะค้นหาไฟล์ `<texture_name>.edds.meta` ในโฟลเดอร์เดียวกันเพื่อดึง GUID แท้จริงมาใส่ให้อัตโนมัติ

---

## 3. การนำไปใช้งานใน DayZ Modding

### โครงสร้างโฟลเดอร์ Mod
```
MyCustomMod/
├── config.cpp
└── Gui/
    ├── imagesets/
    │   ├── my_icons.imageset
    │   ├── my_icons.edds
    │   └── my_icons.edds.meta
    └── layouts/
        └── my_custom_ui.layout
```

### ตัวอย่างโค้ด Enforce Script (.c)
```c
// เรียกแสดงไอคอนบน ImageWidget
ImageWidget icon = ImageWidget.Cast(layout_root.FindAnyWidget("ItemIcon"));
if (icon)
{
    // รูปแบบ: "set:<imageset_name> image:<sprite_name>"
    icon.LoadImageFile(0, "set:my_icons image:battery_icon");
    
    // ย้อมสีตามต้องการ (ARGB: Alpha, Red, Green, Blue)
    icon.SetColor(ARGB(255, 255, 255, 255));
}
```

---

## 4. โครงสร้างโค้ดและการต่อยอดสำหรับนักพัฒนา

- พัฒนาด้วยภาษา **Rust**
- ใช้ GUI Framework: **`eframe` / `egui`** แบบ Immediate Mode ที่กินทรัพยากรเครื่องน้อยมากและลื่นไหลสูง
- จุดสำคัญในโค้ด `src/main.rs`:
  - `parse_imageset(&str) -> ImageSet`: ระบบ Parser สำหรับอ่านไฟล์ภาษา Enfusion syntax
  - `generate_imageset(&ImageSet) -> String`: ตัวสร้างโค้ด DayZ `.imageset` ที่สะอาดและเป็นไปตามมาตรฐาน
  - `convert_edds_to_png`: ตัวแปลงสัญญาณและแคชภาพของไฟล์ `.edds`
