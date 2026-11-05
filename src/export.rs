//! Lossless export. PDF pages are the renderer's SVG converted to vector PDF
//! (text stays text, with the bundled Inter embedded). PPTX slides are built
//! from native shapes, text boxes and tables, with charts embedded as SVG
//! (vector, text outlined) plus a high-resolution PNG fallback.

use resvg::usvg;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;

fn usvg_options() -> usvg::Options<'static> {
    let mut o = usvg::Options { font_family: "Inter".into(), ..Default::default() };
    o.fontdb = crate::fonts::database();
    o
}

// ------------------------------------------------------------------ PDF

pub fn pdf(pages: &[String], width: f64, height: f64, title: &str) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, TextStr};
    // svg2pdf and resvg share the same usvg version
    let opts = usvg_options();
    let mut alloc = Ref::new(1);
    let catalog = alloc.bump();
    let tree_id = alloc.bump();
    let info = alloc.bump();
    let mut pdf = Pdf::new();
    let mut page_ids = Vec::new();
    let mut chunks = Vec::new();
    for (i, svg) in pages.iter().enumerate() {
        let tree = svg2pdf::usvg::Tree::from_str(svg, &opts).map_err(|e| format!("Page {}: {e}", i + 1))?;
        let conv = svg2pdf::ConversionOptions { raster_scale: 3.0, ..Default::default() };
        let (chunk, svg_ref) = svg2pdf::to_chunk(&tree, conv).map_err(|e| format!("Page {}: {e:?}", i + 1))?;
        let mut map = HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let svg_ref = map[&svg_ref];
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let name = Name(b"S1");
        let mut content = Content::new();
        content.transform([width as f32, 0.0, 0.0, height as f32, 0.0, 0.0]).x_object(name);
        pdf.stream(content_id, &content.finish());
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, width as f32, height as f32));
        page.parent(tree_id);
        page.contents(content_id);
        page.resources().x_objects().pair(name, svg_ref);
        page.finish();
        page_ids.push(page_id);
        chunks.push(chunk);
    }
    pdf.catalog(catalog).pages(tree_id);
    pdf.pages(tree_id).kids(page_ids.iter().copied()).count(page_ids.len() as i32);
    pdf.document_info(info).title(TextStr(title)).creator(TextStr("Quire")).producer(TextStr("Quire"));
    for c in &chunks {
        pdf.extend(c);
    }
    Ok(pdf.finish())
}

// ------------------------------------------------------------------ PPTX

#[derive(Deserialize, Debug)]
pub struct Run {
    pub text: String,
    #[serde(default = "default_size")]
    pub size: f64,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub italic: bool,
    /// Letter spacing in points.
    #[serde(default)]
    pub spacing: f64,
}

fn default_pad() -> f64 {
    5.0
}

fn default_size() -> f64 {
    10.0
}

#[derive(Deserialize, Debug)]
pub struct Para {
    pub runs: Vec<Run>,
    #[serde(default)]
    pub bullet: bool,
    #[serde(default)]
    pub align: Option<String>,
    /// Line height in points.
    #[serde(default)]
    pub line_h: Option<f64>,
    /// Space before, in points.
    #[serde(default)]
    pub before: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub struct Cell {
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub fill: Option<String>,
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub border: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub size: Option<f64>,
}

#[derive(Deserialize, Debug)]
pub struct Row {
    pub h: f64,
    pub cells: Vec<Cell>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum El {
    Shape {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<String>,
        /// Outline width in points (0.75 when unset).
        #[serde(default)]
        stroke_w: f64,
        #[serde(default)]
        radius: f64,
    },
    Text {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        paras: Vec<Para>,
        #[serde(default)]
        name: Option<String>,
    },
    Table {
        x: f64,
        y: f64,
        w: f64,
        cols: Vec<f64>,
        rows: Vec<Row>,
        #[serde(default = "default_size")]
        size: f64,
        #[serde(default)]
        name: Option<String>,
        /// space at a cell's sides, in points (the table's Cell padding)
        #[serde(default = "default_pad")]
        pad: f64,
    },
    Svg {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        svg: String,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Deserialize, Debug)]
pub struct Slide {
    pub elements: Vec<El>,
    #[serde(default)]
    pub background: Option<String>,
}

const EMU: f64 = 12700.0;

fn emu(pt: f64) -> i64 {
    (pt * EMU).round() as i64
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' => {}
            c => o.push(c),
        }
    }
    o
}

fn hex(c: &Option<String>, default: &str) -> String {
    let s = c.as_deref().unwrap_or(default).trim_start_matches('#');
    let s = if s.len() == 3 { s.chars().flat_map(|c| [c, c]).collect() } else { s.to_string() };
    if s.len() >= 6 && s[..6].chars().all(|c| c.is_ascii_hexdigit()) {
        s[..6].to_uppercase()
    } else {
        default.trim_start_matches('#').to_uppercase()
    }
}

fn xfrm(x: f64, y: f64, w: f64, h: f64) -> String {
    format!(r#"<a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#, emu(x), emu(y), emu(w.max(0.5)), emu(h.max(0.5)))
}

fn run_xml(r: &Run) -> String {
    let face = esc(r.font.as_deref().filter(|f| !f.is_empty()).unwrap_or("Inter"));
    format!(
        r#"<a:r><a:rPr lang="en-US" sz="{}"{}{}{} dirty="0"><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{face}"/><a:cs typeface="{face}"/></a:rPr><a:t>{}</a:t></a:r>"#,
        (r.size * 100.0).round() as i64,
        if r.bold { r#" b="1""# } else { "" },
        if r.italic { r#" i="1""# } else { "" },
        if r.spacing.abs() > 0.01 { format!(r#" spc="{}""#, (r.spacing * 100.0).round() as i64) } else { String::new() },
        hex(&r.color, "0B0B0B"),
        esc(&r.text)
    )
}

fn para_xml(p: &Para) -> String {
    let algn = match p.align.as_deref() {
        Some("right") => "r",
        Some("center") => "ctr",
        _ => "l",
    };
    let size = p.runs.first().map(|r| r.size).unwrap_or(10.0);
    let mut ppr = format!(r#"<a:pPr algn="{algn}""#);
    if p.bullet {
        let ind = emu(size * 1.1);
        ppr.push_str(&format!(r#" marL="{ind}" indent="-{ind}""#));
    }
    ppr.push('>');
    if let Some(lh) = p.line_h {
        ppr.push_str(&format!(r#"<a:lnSpc><a:spcPts val="{}"/></a:lnSpc>"#, (lh * 100.0).round() as i64));
    }
    if let Some(b) = p.before {
        ppr.push_str(&format!(r#"<a:spcBef><a:spcPts val="{}"/></a:spcBef>"#, (b * 100.0).round() as i64));
    }
    if p.bullet {
        ppr.push_str(r#"<a:buFont typeface="Arial"/><a:buChar char="•"/>"#);
    } else {
        ppr.push_str("<a:buNone/>");
    }
    ppr.push_str("</a:pPr>");
    let runs: String = p.runs.iter().map(run_xml).collect();
    let end = format!(r#"<a:endParaRPr lang="en-US" sz="{}" dirty="0"/>"#, (size * 100.0).round() as i64);
    format!("<a:p>{ppr}{runs}{end}</a:p>")
}

struct SlideOut {
    xml: String,
    rels: Vec<(String, String, String)>, // (rId, type, target)
}

struct Media {
    files: Vec<(String, Vec<u8>)>,
}

fn rasterize(svg: &str, scale: f32) -> Result<(String, Vec<u8>), String> {
    let tree = usvg::Tree::from_str(svg, &usvg_options()).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("Image too large")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let png = pm.encode_png().map_err(|e| e.to_string())?;
    // outline text so the slide looks the same without Inter installed
    let outlined = tree.to_string(&usvg::WriteOptions::default());
    Ok((outlined, png))
}

fn slide_xml(slide: &Slide, idx: usize, media: &mut Media) -> Result<SlideOut, String> {
    let mut shapes = String::new();
    let mut rels = vec![(
        "rId1".to_string(),
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout".to_string(),
        "../slideLayouts/slideLayout1.xml".to_string(),
    )];
    let mut id = 2;
    for el in &slide.elements {
        id += 1;
        match el {
            El::Shape { x, y, w, h, fill, stroke, stroke_w, radius } => {
                let geom = if *radius > 0.0 {
                    let adj = ((radius / w.min(*h).max(1.0)) * 100000.0).min(50000.0).round() as i64;
                    format!(r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#)
                } else {
                    r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_string()
                };
                // "none" / "transparent": an outline without a fill
                let fill = match fill.as_deref().filter(|f| !matches!(*f, "none" | "transparent")) {
                    Some(_) => format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(fill, "FFFFFF")),
                    None => "<a:noFill/>".into(),
                };
                let ln = match stroke {
                    Some(_) => format!(r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#, emu(if *stroke_w > 0.0 { *stroke_w } else { 0.75 }), hex(stroke, "DDDDDD")),
                    None => "<a:ln><a:noFill/></a:ln>".into(),
                };
                shapes.push_str(&format!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Card {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}{geom}{fill}{ln}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp>"#,
                    xfrm(*x, *y, *w, *h)
                ));
            }
            El::Text { x, y, w, h, paras, name } => {
                let body: String = paras.iter().map(para_xml).collect();
                shapes.push_str(&format!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"><a:noAutofit/></a:bodyPr><a:lstStyle/>{body}</p:txBody></p:sp>"#,
                    esc(name.as_deref().unwrap_or("Text")),
                    // a little extra width absorbs metric differences
                    xfrm(*x, *y, *w * 1.03, *h)
                ));
            }
            El::Table { x, y, w, cols, rows, size, name, pad } => {
                // the cell padding, in proportion to what tables always had here (3.6 pt at the default 5 pt)
                let mar = (45720.0 * pad.max(0.0) / 5.0).round() as i64;
                let total: f64 = cols.iter().sum::<f64>().max(1.0);
                let grid: String =
                    cols.iter().map(|c| format!(r#"<a:gridCol w="{}"/>"#, emu(c / total * w))).collect();
                let mut trs = String::new();
                let mut height = 0.0;
                for r in rows {
                    height += r.h;
                    let mut tcs = String::new();
                    for c in &r.cells {
                        let algn = match c.align.as_deref() {
                            Some("right") => "r",
                            Some("center") => "ctr",
                            _ => "l",
                        };
                        let run = Run { text: c.text.clone(), size: c.size.unwrap_or(*size), bold: c.bold, color: c.color.clone(), font: c.font.clone(), italic: c.italic, spacing: 0.0 };
                        let fill = match &c.fill {
                            Some(_) => format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(&c.fill, "FFFFFF")),
                            None => "<a:noFill/>".into(),
                        };
                        let line = |tag: &str, col: &Option<String>| match col {
                            Some(_) => format!(
                                r#"<a:{tag} w="6350"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:{tag}>"#,
                                hex(col, "E1E0D9")
                            ),
                            None => format!(r#"<a:{tag} w="0"><a:noFill/></a:{tag}>"#),
                        };
                        tcs.push_str(&format!(
                            r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn="{algn}"/>{}</a:p></a:txBody><a:tcPr marL="{mar}" marR="{mar}" marT="0" marB="0" anchor="ctr">{}{}{}{}{fill}</a:tcPr></a:tc>"#,
                            run_xml(&run),
                            line("lnL", &None),
                            line("lnR", &None),
                            line("lnT", &None),
                            line("lnB", &c.border),
                        ));
                    }
                    trs.push_str(&format!(r#"<a:tr h="{}">{tcs}</a:tr>"#, emu(r.h)));
                }
                shapes.push_str(&format!(
                    r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="{}"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr/><a:tblGrid>{grid}</a:tblGrid>{trs}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
                    esc(name.as_deref().unwrap_or("Table")),
                    emu(*x),
                    emu(*y),
                    emu(*w),
                    emu(height)
                ));
            }
            El::Svg { x, y, w, h, svg, name } => {
                let (outlined, png) = rasterize(svg, 4.0)?;
                let n = media.files.len() / 2 + 1;
                let svg_name = format!("image{idx}_{n}.svg");
                let png_name = format!("image{idx}_{n}.png");
                media.files.push((svg_name.clone(), outlined.into_bytes()));
                media.files.push((png_name.clone(), png));
                let rp = format!("rId{}", rels.len() + 1);
                rels.push((rp.clone(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image".into(), format!("../media/{png_name}")));
                let rs = format!("rId{}", rels.len() + 1);
                rels.push((rs.clone(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image".into(), format!("../media/{svg_name}")));
                shapes.push_str(&format!(
                    r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rp}"><a:extLst><a:ext uri="{{96DAC541-7B7A-43D3-8B79-37D633B846F1}}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="{rs}"/></a:ext></a:extLst></a:blip><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
                    esc(name.as_deref().unwrap_or("Chart")),
                    xfrm(*x, *y, *w, *h)
                ));
            }
        }
    }
    let bg = match &slide.background {
        Some(_) => format!(
            r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
            hex(&slide.background, "FFFFFF")
        ),
        None => String::new(),
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld>{bg}<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    );
    Ok(SlideOut { xml, rels })
}

fn rels_xml(rels: &[(String, String, String)]) -> String {
    let body: String = rels
        .iter()
        .map(|(id, t, target)| format!(r#"<Relationship Id="{id}" Type="{t}" Target="{target}"/>"#))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Quire"><a:themeElements><a:clrScheme name="Quire"><a:dk1><a:srgbClr val="0B0B0B"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="52514E"/></a:dk2><a:lt2><a:srgbClr val="F9F9F7"/></a:lt2><a:accent1><a:srgbClr val="2A78D6"/></a:accent1><a:accent2><a:srgbClr val="EB6834"/></a:accent2><a:accent3><a:srgbClr val="1BAF7A"/></a:accent3><a:accent4><a:srgbClr val="EDA100"/></a:accent4><a:accent5><a:srgbClr val="E87BA4"/></a:accent5><a:accent6><a:srgbClr val="008300"/></a:accent6><a:hlink><a:srgbClr val="2A78D6"/></a:hlink><a:folHlink><a:srgbClr val="4A3AA7"/></a:folHlink></a:clrScheme><a:fontScheme name="Quire"><a:majorFont><a:latin typeface="Inter"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Inter"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Quire"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

const MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz="2400"><a:latin typeface="Inter"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr><a:defRPr sz="1200"><a:latin typeface="Inter"/></a:defRPr></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz="1000"><a:latin typeface="Inter"/></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#;

const LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;

pub fn pptx(slides: &[Slide], width: f64, height: f64, title: &str) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buf);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let put = |zip: &mut zip::ZipWriter<&mut std::io::Cursor<Vec<u8>>>, name: &str, data: &[u8]| -> Result<(), String> {
        zip.start_file(name, opts).map_err(|e| e.to_string())?;
        zip.write_all(data).map_err(|e| e.to_string())
    };
    const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
    let n = slides.len();
    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="svg" ContentType="image/svg+xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/><Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/><Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    for i in 1..=n {
        ct.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    ct.push_str("</Types>");
    put(&mut zip, "[Content_Types].xml", ct.as_bytes())?;
    put(
        &mut zip,
        "_rels/.rels",
        rels_xml(&[
            ("rId1".into(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument".into(), "ppt/presentation.xml".into()),
            ("rId2".into(), "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties".into(), "docProps/core.xml".into()),
            ("rId3".into(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties".into(), "docProps/app.xml".into()),
        ])
        .as_bytes(),
    )?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    put(
        &mut zip,
        "docProps/core.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{}</dc:title><dc:creator>Quire</dc:creator><dcterms:created xsi:type="dcterms:W3CDTF">{now}</dcterms:created><dcterms:modified xsi:type="dcterms:W3CDTF">{now}</dcterms:modified></cp:coreProperties>"#,
            esc(title)
        )
        .as_bytes(),
    )?;
    put(
        &mut zip,
        "docProps/app.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Quire</Application><Slides>{n}</Slides></Properties>"#
        )
        .as_bytes(),
    )?;

    let mut pres_rels = vec![(
        "rId1".to_string(),
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster".to_string(),
        "slideMasters/slideMaster1.xml".to_string(),
    )];
    let mut sld_ids = String::new();
    for i in 1..=n {
        let rid = format!("rId{}", i + 1);
        sld_ids.push_str(&format!(r#"<p:sldId id="{}" r:id="{rid}"/>"#, 255 + i));
        pres_rels.push((rid, REL_SLIDE.into(), format!("slides/slide{i}.xml")));
    }
    for (k, (t, f)) in [
        ("presProps", "presProps.xml"),
        ("viewProps", "viewProps.xml"),
        ("theme", "theme/theme1.xml"),
        ("tableStyles", "tableStyles.xml"),
    ]
    .iter()
    .enumerate()
    {
        pres_rels.push((
            format!("rId{}", n + 2 + k),
            format!("http://schemas.openxmlformats.org/officeDocument/2006/relationships/{t}"),
            f.to_string(),
        ));
    }
    put(
        &mut zip,
        "ppt/presentation.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{sld_ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle><a:lvl1pPr><a:defRPr sz="1000"><a:latin typeface="Inter"/></a:defRPr></a:lvl1pPr></p:defaultTextStyle></p:presentation>"#,
            emu(width),
            emu(height)
        )
        .as_bytes(),
    )?;
    put(&mut zip, "ppt/_rels/presentation.xml.rels", rels_xml(&pres_rels).as_bytes())?;
    put(
        &mut zip,
        "ppt/presProps.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#,
    )?;
    put(
        &mut zip,
        "ppt/viewProps.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:viewPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:normalViewPr><p:restoredLeft sz="15620"/><p:restoredTop sz="94660"/></p:normalViewPr><p:gridSpacing cx="76200" cy="76200"/></p:viewPr>"#,
    )?;
    put(
        &mut zip,
        "ppt/tableStyles.xml",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#,
    )?;
    put(&mut zip, "ppt/theme/theme1.xml", THEME.as_bytes())?;
    put(&mut zip, "ppt/slideMasters/slideMaster1.xml", MASTER.as_bytes())?;
    put(
        &mut zip,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels_xml(&[
            ("rId1".into(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout".into(), "../slideLayouts/slideLayout1.xml".into()),
            ("rId2".into(), "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme".into(), "../theme/theme1.xml".into()),
        ])
        .as_bytes(),
    )?;
    put(&mut zip, "ppt/slideLayouts/slideLayout1.xml", LAYOUT.as_bytes())?;
    put(
        &mut zip,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        rels_xml(&[(
            "rId1".into(),
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster".into(),
            "../slideMasters/slideMaster1.xml".into(),
        )])
        .as_bytes(),
    )?;

    let mut media = Media { files: Vec::new() };
    for (i, s) in slides.iter().enumerate() {
        let out = slide_xml(s, i + 1, &mut media)?;
        put(&mut zip, &format!("ppt/slides/slide{}.xml", i + 1), out.xml.as_bytes())?;
        put(&mut zip, &format!("ppt/slides/_rels/slide{}.xml.rels", i + 1), rels_xml(&out.rels).as_bytes())?;
    }
    for (name, data) in &media.files {
        put(&mut zip, &format!("ppt/media/{name}"), data)?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100"><rect x="10" y="10" width="80" height="40" fill="#2a78d6"/><text x="10" y="80" font-family="Inter" font-size="12">Revenue €1.2M — Выручка</text></svg>"##;

    #[test]
    fn pdf_has_pages_and_embedded_text() {
        let bytes = pdf(&[SVG.to_string(), SVG.to_string()], 200.0, 100.0, "Test").unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.starts_with("%PDF"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("Inter"), "font is embedded");
    }

    #[test]
    fn pptx_is_a_valid_package() {
        let slide = Slide {
            background: Some("#ffffff".into()),
            elements: vec![
                El::Shape { x: 10.0, y: 10.0, w: 100.0, h: 50.0, fill: Some("#fcfcfb".into()), stroke: Some("#e1e0d9".into()), stroke_w: 0.0, radius: 6.0 },
                El::Text {
                    x: 12.0,
                    y: 12.0,
                    w: 90.0,
                    h: 20.0,
                    name: None,
                    paras: vec![Para { runs: vec![Run { text: "Hello & <world>".into(), size: 10.0, bold: true, color: None, font: None, italic: false, spacing: 0.0 }], bullet: false, align: None, line_h: Some(14.0), before: None }],
                },
                El::Table {
                    x: 10.0,
                    y: 70.0,
                    w: 120.0,
                    cols: vec![2.0, 1.0],
                    size: 8.0,
                    name: None,
                    pad: 5.0,
                    rows: vec![Row { h: 16.0, cells: vec![Cell { text: "A".into(), bold: true, color: None, fill: None, align: None, border: Some("#ccc".into()), font: None, italic: false, size: None }, Cell { text: "1".into(), bold: false, color: None, fill: None, align: Some("right".into()), border: None, font: None, italic: false, size: None }] }],
                },
                El::Svg { x: 0.0, y: 100.0, w: 200.0, h: 100.0, svg: SVG.into(), name: None },
            ],
        };
        let bytes = pptx(&[slide], 842.0, 595.0, "T").unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"ppt/slides/slide1.xml".to_string()));
        assert!(names.iter().any(|n| n.ends_with(".svg")));
        assert!(names.iter().any(|n| n.ends_with(".png")));
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut z.by_name("ppt/slides/slide1.xml").unwrap(), &mut xml).unwrap();
        assert!(xml.contains("Hello &amp; &lt;world&gt;"));
        roxml_check(&xml);
    }

    fn roxml_check(xml: &str) {
        usvg::roxmltree::Document::parse(xml).expect("slide XML is well-formed");
    }
}
