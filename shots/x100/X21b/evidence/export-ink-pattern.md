# X21b 导出同源取证（ink 主题：同一份 Scene、同一份 Theme）

由 `MW_X21B_EVIDENCE=1 cargo test -p mw-shell svg_and_pdf_agree` 导出的真实产物，原始文件见同目录 `export-ink.svg` / `export-ink.pdf`。

## SVG
```xml
<defs><pattern id="zdots" width="28" height="28" patternUnits="userSpaceOnUse"><circle cx="14" cy="14" r="1.1" fill="#3c424a"/></pattern></defs>
```

## PDF 图案对象（整份 PDF 只有一个）
```
<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 28 28] /XStep 28 /YStep 28 /Matrix [1 0 0 1 0 0] /Resources << >> /Length 186 >> stream q 0.24 0.26 0.29 rg 0.24 0.26 0.29 RG 0.2 w 15.1 14 m 15.1 14.61 14.61 15.1 14 15.1 c 13.39 15.1 12
```

## PDF 图案内容流
```
q
0.24 0.26 0.29 rg
0.24 0.26 0.29 RG 0.2 w
15.1 14 m
15.1 14.61 14.61 15.1 14 15.1 c
13.39 15.1 12.9 14.61 12.9 14 c
12.9 13.39 13.39 12.9 14 12.9 c
14.61 12.9 15.1 13.39 15.1 14 c
f
Q
```

## PDF 页资源与画笔
```
/Resources << /Pattern << /P1 N 0 R >> >>
/Pattern cs /P1 scn    ← 每页内容流一次，铺满本页内容窗口
```
