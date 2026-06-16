// Included from docx.rs: the Converter method bodies.

impl<'a> Converter<'a> {
    fn load_part(&self, path: &str) -> Option<Vec<u8>> {
        let e = self.zip.find(path)?;
        self.zip.extract(e)
    }

    // ---- part loading / lookup tables ----

    fn find_document_path(&self) -> String {
        if let Some(buf) = self.load_part("_rels/.rels") {
            let s = String::from_utf8_lossy(&buf);
            let mut xp = XmlParser::new(&s);
            loop {
                match xp.next() {
                    Event::Eof => break,
                    Event::Start if xp.name() == "Relationship" => {
                        let ty = xp.attr("Type");
                        if ty.len() >= 15 && &ty[ty.len() - 15..] == "/officeDocument" {
                            let target = decode_attr(xp.attr("Target"));
                            return resolve_zip_path("", &target);
                        }
                    }
                    _ => {}
                }
            }
        }
        "word/document.xml".to_string()
    }

    fn parse_rels(&mut self, xml: &str) {
        let mut xp = XmlParser::new(xml);
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::Start if xp.name() == "Relationship" => {
                    let target = decode_attr(xp.attr("Target"));
                    let external = xp.attr("TargetMode") == "External";
                    let id = xp.attr("Id").to_string();
                    self.rels.entry(id).or_insert(Relationship { target, external });
                }
                _ => {}
            }
        }
    }

    fn parse_styles(&mut self, xml: &str) {
        let mut xp = XmlParser::new(xml);
        let mut cur_id = String::new();
        let mut cur_type = String::new();
        loop {
            let ev = xp.next();
            match ev {
                Event::Eof => break,
                Event::End => {
                    if xp.name() == "w:style" {
                        cur_id.clear();
                    }
                    continue;
                }
                Event::Start => {}
                _ => continue,
            }
            let n = xp.name();
            if n == "w:style" {
                cur_id = xp.attr("w:styleId").to_string();
                cur_type = xp.attr("w:type").to_string();
                let cb = cur_id.as_bytes();
                if cur_type == "paragraph"
                    && cur_id.len() == 8
                    && cur_id.starts_with("Heading")
                    && (b'1'..=b'6').contains(&cb[7])
                {
                    self.heading_by_style.insert(cur_id.clone(), (cb[7] - b'0') as i32);
                }
                continue;
            }
            if cur_id.is_empty() {
                continue;
            }
            if cur_type == "paragraph" {
                if n == "w:name" {
                    let nm = to_lower(xp.attr("w:val"));
                    let nb = nm.as_bytes();
                    let lvl = if nm == "title" {
                        1
                    } else if nm.len() == 9 && nm.starts_with("heading ") && (b'1'..=b'6').contains(&nb[8]) {
                        (nb[8] - b'0') as i32
                    } else {
                        0
                    };
                    if lvl != 0 {
                        self.heading_by_style.insert(cur_id.clone(), lvl);
                    }
                } else if n == "w:outlineLvl" {
                    let lvl = sv_to_int(xp.attr("w:val"));
                    if (0..=5).contains(&lvl) && !self.heading_by_style.contains_key(&cur_id) {
                        self.heading_by_style.insert(cur_id.clone(), lvl + 1);
                    }
                } else if n == "w:numId" {
                    self.style_num.entry(cur_id.clone()).or_insert(StyleNum { num_id: -1, ilvl: 0 }).num_id =
                        sv_to_int(xp.attr("w:val"));
                } else if n == "w:ilvl" {
                    self.style_num.entry(cur_id.clone()).or_insert(StyleNum { num_id: -1, ilvl: 0 }).ilvl =
                        sv_to_int(xp.attr("w:val"));
                }
            } else if cur_type == "character" {
                let val = xp.attr("w:val").to_string();
                apply_run_prop(self.char_style.entry(cur_id.clone()).or_default(), n, &val);
            }
        }
    }

    fn parse_numbering(&mut self, xml: &str) {
        let mut xp = XmlParser::new(xml);
        let mut cur_abstract = -1i32;
        let mut cur_lvl = -1i32;
        let mut cur_num = -1i32;
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::Start => {
                    let n = xp.name();
                    if n == "w:abstractNum" {
                        cur_abstract = sv_to_int(xp.attr("w:abstractNumId"));
                    } else if n == "w:lvl" || n == "w:lvlOverride" {
                        cur_lvl = sv_to_int(xp.attr("w:ilvl"));
                    } else if n == "w:numFmt" && cur_lvl >= 0 {
                        if cur_abstract >= 0 {
                            self.abstract_fmt.entry(cur_abstract).or_default().insert(cur_lvl, xp.attr("w:val").to_string());
                        } else if cur_num >= 0 {
                            self.num_fmt_override.entry(cur_num).or_default().insert(cur_lvl, xp.attr("w:val").to_string());
                        }
                    } else if (n == "w:start" || n == "w:startOverride") && cur_lvl >= 0 {
                        if cur_abstract >= 0 {
                            self.abstract_start.entry(cur_abstract).or_default().insert(cur_lvl, sv_to_int(xp.attr("w:val")));
                        } else if cur_num >= 0 {
                            self.num_start_override.entry(cur_num).or_default().insert(cur_lvl, sv_to_int(xp.attr("w:val")));
                        }
                    } else if n == "w:num" {
                        cur_num = sv_to_int(xp.attr("w:numId"));
                    } else if n == "w:abstractNumId" && cur_num >= 0 {
                        self.num_to_abstract.insert(cur_num, sv_to_int(xp.attr("w:val")));
                    }
                }
                Event::End => {
                    let n = xp.name();
                    if n == "w:abstractNum" {
                        cur_abstract = -1;
                        cur_lvl = -1;
                    } else if n == "w:lvl" || n == "w:lvlOverride" {
                        cur_lvl = -1;
                    } else if n == "w:num" {
                        cur_num = -1;
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_core_props(&mut self, xml: &str) {
        let mut xp = XmlParser::new(xml);
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::Start if xp.name() == "dc:title" => {
                    let mut raw = String::new();
                    loop {
                        match xp.next() {
                            Event::Eof | Event::End => break,
                            Event::Text => raw.push_str(xp.text()),
                            _ => {}
                        }
                    }
                    let mut t = String::new();
                    XmlParser::append_decoded(&raw, &mut t);
                    self.title = t;
                    return;
                }
                _ => {}
            }
        }
    }

    // ---- list state ----

    fn num_fmt_for(&self, num_id: i32, ilvl: i32) -> String {
        if let Some(m) = self.num_fmt_override.get(&num_id) {
            if let Some(v) = m.get(&ilvl) {
                return v.clone();
            }
        }
        if let Some(&a) = self.num_to_abstract.get(&num_id) {
            if let Some(m) = self.abstract_fmt.get(&a) {
                if let Some(v) = m.get(&ilvl) {
                    return v.clone();
                }
            }
        }
        "decimal".to_string()
    }

    fn start_for(&self, num_id: i32, ilvl: i32) -> i32 {
        if let Some(m) = self.num_start_override.get(&num_id) {
            if let Some(&v) = m.get(&ilvl) {
                return v;
            }
        }
        if let Some(&a) = self.num_to_abstract.get(&num_id) {
            if let Some(m) = self.abstract_start.get(&a) {
                if let Some(&v) = m.get(&ilvl) {
                    return v;
                }
            }
        }
        1
    }

    fn close_one_list(&mut self) {
        let top = self.list_stack.pop().unwrap();
        if top.li_open {
            self.out.push_str("</li>");
        }
        self.out.push_str("</");
        self.out.push_str(top.tag);
        self.out.push('>');
    }

    fn close_lists(&mut self, target: usize) {
        while self.list_stack.len() > target {
            self.close_one_list();
        }
    }

    fn set_list_state(&mut self, num_id: i32, ilvl: i32) {
        let target = ilvl as usize + 1;
        let fmt = self.num_fmt_for(num_id, ilvl);
        let tag = if fmt == "bullet" || fmt == "none" { "ul" } else { "ol" };
        self.close_lists(target);
        if self.list_stack.len() == target && self.list_stack.last().unwrap().tag != tag {
            self.close_one_list();
        }
        while self.list_stack.len() < target {
            let lvl = self.list_stack.len() as i32;
            let f = self.num_fmt_for(num_id, lvl);
            let t = if f == "bullet" || f == "none" { "ul" } else { "ol" };
            self.out.push('<');
            self.out.push_str(t);
            match f.as_str() {
                "lowerLetter" => self.out.push_str(" type=\"a\""),
                "upperLetter" => self.out.push_str(" type=\"A\""),
                "lowerRoman" => self.out.push_str(" type=\"i\""),
                "upperRoman" => self.out.push_str(" type=\"I\""),
                "none" => self.out.push_str(" style=\"list-style:none\""),
                _ => {}
            }
            if t.as_bytes()[0] == b'o' {
                let start_at = self.start_for(num_id, lvl) + self.list_emitted.get(&list_key(num_id, lvl)).copied().unwrap_or(0);
                if start_at != 1 {
                    self.out.push_str(&format!(" start=\"{start_at}\""));
                }
            }
            self.out.push('>');
            self.list_stack.push(OpenList { tag: t, li_open: false });
        }
    }

    // ---- document traversal ----

    fn convert_body(&mut self, xml: &str) {
        let mut xp = XmlParser::new(xml);
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::Start if xp.name() == "w:body" => {
                    self.parse_blocks(&mut xp, "w:body", 0);
                    break;
                }
                _ => {}
            }
        }
        self.close_lists(0);
    }

    fn parse_blocks(&mut self, xp: &mut XmlParser, end_name: &str, depth: i32) {
        loop {
            match xp.next() {
                Event::Eof => return,
                Event::End if xp.name() == end_name => return,
                Event::Start => {
                    let name = xp.name().to_string();
                    self.parse_block_child(xp, &name, depth);
                }
                _ => {}
            }
        }
    }

    fn parse_block_child(&mut self, xp: &mut XmlParser, name: &str, depth: i32) {
        if depth > MAX_DEPTH {
            xp.skip_element();
            return;
        }
        let d = depth + 1;
        if name == "w:p" {
            self.parse_paragraph(xp, d);
        } else if name == "w:tbl" {
            self.close_lists(0);
            self.parse_table(xp, d);
        } else if matches!(name, "w:sectPr" | "w:sdtPr" | "w:sdtEndPr" | "w:del" | "w:moveFrom") {
            xp.skip_element();
        } else {
            self.parse_blocks(xp, name, d);
        }
    }

    fn parse_paragraph(&mut self, xp: &mut XmlParser, depth: i32) {
        let mut pp = ParaProps::new();
        let mut close_tag = String::new();
        let mut opened = false;
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::End if xp.name() == "w:p" => break,
                Event::Start => {
                    let n = xp.name().to_string();
                    if n == "w:pPr" {
                        self.parse_para_props(xp, &mut pp);
                        if pp.num_id < 0 && !pp.style_id.is_empty() {
                            if let Some(sn) = self.style_num.get(&pp.style_id) {
                                pp.num_id = sn.num_id;
                                pp.ilvl = sn.ilvl;
                            }
                        }
                        continue;
                    }
                    if !opened {
                        close_tag = self.open_paragraph(&pp);
                        opened = true;
                    }
                    self.parse_inline(xp, &n, depth);
                }
                _ => {}
            }
        }
        for f in &mut self.fields {
            if f.open {
                self.out.push_str("</a>");
                f.open = false;
            }
        }
        if !opened {
            close_tag = self.open_paragraph(&pp);
        }
        self.out.push_str(&close_tag);
    }

    fn parse_para_props(&mut self, xp: &mut XmlParser, pp: &mut ParaProps) {
        let mut depth = 1;
        while depth > 0 {
            match xp.next() {
                Event::Eof => return,
                Event::Start => {
                    let n = xp.name();
                    if n == "w:pStyle" {
                        pp.style_id = xp.attr("w:val").to_string();
                        if let Some(&h) = self.heading_by_style.get(&pp.style_id) {
                            pp.heading = h;
                        }
                    } else if n == "w:jc" {
                        pp.align = map_align(xp.attr("w:val"));
                    } else if n == "w:ilvl" {
                        pp.ilvl = sv_to_int(xp.attr("w:val"));
                    } else if n == "w:numId" {
                        pp.num_id = sv_to_int(xp.attr("w:val"));
                    } else if n == "w:ind" {
                        let mut left = xp.attr("w:left");
                        if left.is_empty() {
                            left = xp.attr("w:start");
                        }
                        let hanging = sv_to_int(xp.attr("w:hanging"));
                        let first_line = sv_to_int(xp.attr("w:firstLine"));
                        append_pt(&mut pp.css, "margin-left", sv_to_int(left));
                        if hanging != 0 {
                            append_pt(&mut pp.css, "text-indent", -hanging);
                        } else if first_line != 0 {
                            append_pt(&mut pp.css, "text-indent", first_line);
                        }
                    } else if n == "w:shd" {
                        let fill = xp.attr("w:fill");
                        if !fill.is_empty() && fill != "auto" {
                            pp.css.push_str("background-color:#");
                            pp.css.push_str(fill);
                            pp.css.push(';');
                        }
                    } else if n == "w:bidi" {
                        pp.rtl = toggle_on(xp.attr("w:val"));
                    }
                    depth += 1;
                }
                Event::End => depth -= 1,
                _ => {}
            }
        }
    }

    fn open_paragraph(&mut self, pp: &ParaProps) -> String {
        if pp.num_id > 0 && pp.heading == 0 {
            self.set_list_state(pp.num_id, pp.ilvl);
            let top = self.list_stack.last_mut().unwrap();
            if top.li_open {
                self.out.push_str("</li>");
            }
            self.out.push_str("<li>");
            top.li_open = true;
            *self.list_emitted.entry(list_key(pp.num_id, pp.ilvl)).or_insert(0) += 1;
            for l in pp.ilvl + 1..=8 {
                self.list_emitted.remove(&list_key(pp.num_id, l));
            }
            return String::new();
        }
        self.close_lists(0);
        let tag = if pp.heading > 0 {
            format!("h{}", pp.heading)
        } else {
            "p".to_string()
        };
        let mut style = String::new();
        if !pp.align.is_empty() {
            style.push_str("text-align:");
            style.push_str(pp.align);
            style.push(';');
        }
        style.push_str(&pp.css);
        self.out.push('<');
        self.out.push_str(&tag);
        if pp.rtl {
            self.out.push_str(" dir=\"rtl\"");
        }
        if !style.is_empty() {
            self.out.push_str(" style=\"");
            self.out.push_str(&style);
            self.out.push('"');
        }
        self.out.push('>');
        format!("</{tag}>")
    }

    fn parse_inline(&mut self, xp: &mut XmlParser, name: &str, depth: i32) {
        if depth > MAX_DEPTH {
            xp.skip_element();
            return;
        }
        let d = depth + 1;
        if name == "w:r" {
            self.parse_run(xp, d);
        } else if name == "w:hyperlink" {
            self.parse_hyperlink(xp, d);
        } else if name == "w:fldSimple" {
            let href = hyperlink_from_instr(&decode_attr(xp.attr("w:instr")));
            if !href.is_empty() {
                self.out.push_str("<a href=\"");
                append_escaped_attr(&href, &mut self.out);
                self.out.push_str("\">");
            }
            self.parse_inline_children(xp, "w:fldSimple", d);
            if !href.is_empty() {
                self.out.push_str("</a>");
            }
        } else if name == "m:t" {
            self.parse_text(xp);
        } else if name == "w:bookmarkStart" {
            let nm = xp.attr("w:name");
            if !nm.is_empty() && nm != "_GoBack" {
                self.out.push_str("<a id=\"");
                append_escaped_attr(&decode_attr(nm), &mut self.out);
                self.out.push_str("\"></a>");
            }
            xp.skip_element();
        } else if name == "mc:AlternateContent" {
            loop {
                match xp.next() {
                    Event::Eof => return,
                    Event::End if xp.name() == "mc:AlternateContent" => return,
                    Event::Start => {
                        if xp.name() == "mc:Fallback" {
                            self.parse_inline_children(xp, "mc:Fallback", d);
                        } else {
                            xp.skip_element();
                        }
                    }
                    _ => {}
                }
            }
        } else if matches!(name, "w:del" | "w:moveFrom" | "w:pPr") {
            xp.skip_element();
        } else {
            self.parse_inline_children(xp, name, d);
        }
    }

    fn parse_inline_children(&mut self, xp: &mut XmlParser, end_name: &str, depth: i32) {
        loop {
            match xp.next() {
                Event::Eof => return,
                Event::End if xp.name() == end_name => return,
                Event::Start => {
                    let name = xp.name().to_string();
                    self.parse_inline(xp, &name, depth);
                }
                _ => {}
            }
        }
    }

    fn parse_run(&mut self, xp: &mut XmlParser, depth: i32) {
        let mut rp = RunProps::default();
        let mut close = String::new();
        let mut opened = false;
        self.parse_run_children(xp, "w:r", &mut rp, &mut opened, &mut close, depth);
        if opened {
            self.out.push_str(&close);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_run_children(&mut self, xp: &mut XmlParser, end_name: &str, rp: &mut RunProps, opened: &mut bool, close: &mut String, depth: i32) {
        loop {
            match xp.next() {
                Event::Eof => return,
                Event::End if xp.name() == end_name => return,
                Event::Start => {}
                _ => continue,
            }
            let n = xp.name().to_string();
            if n == "w:rPr" {
                self.parse_run_props(xp, rp);
                continue;
            }
            if n == "w:fldChar" {
                self.handle_fld_char(xp);
                continue;
            }
            if n == "w:instrText" {
                self.handle_instr_text(xp);
                continue;
            }
            if matches!(n.as_str(), "w:delText" | "w:lastRenderedPageBreak" | "w:footnoteRef" | "w:endnoteRef" | "w:separator" | "w:continuationSeparator" | "w:commentReference" | "w:annotationRef") {
                xp.skip_element();
                continue;
            }
            if n == "mc:AlternateContent" {
                loop {
                    match xp.next() {
                        Event::Eof => return,
                        Event::End if xp.name() == "mc:AlternateContent" => break,
                        Event::Start => {
                            if xp.name() == "mc:Fallback" {
                                self.parse_run_children(xp, "mc:Fallback", rp, opened, close, depth);
                            } else {
                                xp.skip_element();
                            }
                        }
                        _ => {}
                    }
                }
                continue;
            }
            let emits = matches!(n.as_str(), "w:t" | "m:t" | "w:br" | "w:cr" | "w:tab" | "w:drawing" | "w:pict" | "w:object" | "w:noBreakHyphen" | "w:softHyphen" | "w:sym" | "w:footnoteReference" | "w:endnoteReference");
            if emits && !*opened {
                let mut open = String::new();
                build_run_tags(rp, &mut open, close);
                self.out.push_str(&open);
                *opened = true;
            }
            if n == "w:t" || n == "m:t" {
                self.parse_text(xp);
            } else if n == "w:br" || n == "w:cr" {
                self.out.push_str("<br>");
                xp.skip_element();
            } else if n == "w:tab" {
                self.out.push_str("<span class=\"tab\">\t</span>");
                xp.skip_element();
            } else if matches!(n.as_str(), "w:drawing" | "w:pict" | "w:object") {
                self.parse_drawing(xp, depth);
            } else if n == "w:noBreakHyphen" {
                self.out.push_str("&#8209;");
                xp.skip_element();
            } else if n == "w:softHyphen" {
                self.out.push_str("&shy;");
                xp.skip_element();
            } else if n == "w:sym" {
                self.emit_sym(xp);
            } else if n == "w:footnoteReference" {
                self.emit_note_ref(xp, true, rp.vert == 1);
            } else if n == "w:endnoteReference" {
                self.emit_note_ref(xp, false, rp.vert == 1);
            } else if depth > MAX_DEPTH {
                xp.skip_element();
            } else {
                self.parse_run_children(xp, &n, rp, opened, close, depth + 1);
            }
        }
    }

    fn parse_run_props(&mut self, xp: &mut XmlParser, rp: &mut RunProps) {
        let mut depth = 1;
        while depth > 0 {
            match xp.next() {
                Event::Eof => return,
                Event::Start => {
                    let n = xp.name();
                    if n == "w:rStyle" {
                        if let Some(base) = self.char_style.get(xp.attr("w:val")) {
                            *rp = base.clone();
                        }
                    } else {
                        let val = xp.attr("w:val").to_string();
                        apply_run_prop(rp, n, &val);
                    }
                    depth += 1;
                }
                Event::End => depth -= 1,
                _ => {}
            }
        }
    }

    fn parse_text(&mut self, xp: &mut XmlParser) {
        loop {
            match xp.next() {
                Event::Eof | Event::End => return,
                Event::Text => self.out.push_str(xp.text()),
                _ => {}
            }
        }
    }

    fn parse_hyperlink(&mut self, xp: &mut XmlParser, depth: i32) {
        let mut href = String::new();
        let rid = xp.attr("r:id").to_string();
        let anchor = xp.attr("w:anchor").to_string();
        let title = decode_attr(xp.attr("w:tooltip"));
        if !rid.is_empty() {
            if let Some(r) = self.rels.get(&rid) {
                href = r.target.clone();
            }
        } else if !anchor.is_empty() {
            href = format!("#{}", decode_attr(&anchor));
        }
        if !href.is_empty() {
            self.out.push_str("<a href=\"");
            append_escaped_attr(&href, &mut self.out);
            if !title.is_empty() {
                self.out.push_str("\" title=\"");
                append_escaped_attr(&title, &mut self.out);
            }
            self.out.push_str("\">");
        }
        self.parse_inline_children(xp, "w:hyperlink", depth);
        if !href.is_empty() {
            self.out.push_str("</a>");
        }
    }

    // ---- fields, notes, symbols ----

    fn handle_fld_char(&mut self, xp: &mut XmlParser) {
        let ty = xp.attr("w:fldCharType").to_string();
        if ty == "begin" {
            self.fields.push(Field { instr: String::new(), collecting: true, open: false });
        } else if ty == "separate" {
            if let Some(f) = self.fields.last_mut() {
                if f.collecting {
                    f.collecting = false;
                    let href = hyperlink_from_instr(&f.instr);
                    if !href.is_empty() {
                        self.out.push_str("<a href=\"");
                        append_escaped_attr(&href, &mut self.out);
                        self.out.push_str("\">");
                        self.fields.last_mut().unwrap().open = true;
                    }
                }
            }
        } else if ty == "end" {
            if let Some(f) = self.fields.pop() {
                if f.open {
                    self.out.push_str("</a>");
                }
            }
        }
        xp.skip_element();
    }

    fn handle_instr_text(&mut self, xp: &mut XmlParser) {
        if self.fields.last().map(|f| !f.collecting).unwrap_or(true) {
            xp.skip_element();
            return;
        }
        let mut collected = String::new();
        loop {
            match xp.next() {
                Event::Eof | Event::End => break,
                Event::Text => XmlParser::append_decoded(xp.text(), &mut collected),
                _ => {}
            }
        }
        if let Some(f) = self.fields.last_mut() {
            f.instr.push_str(&collected);
        }
    }

    fn emit_sym(&mut self, xp: &mut XmlParser) {
        let font = decode_attr(xp.attr("w:font"));
        let cp = sv_to_hex(xp.attr("w:char"));
        xp.skip_element();
        if cp == 0 {
            return;
        }
        let ent = format!("&#x{cp:X};");
        if !font.is_empty() {
            self.out.push_str("<span style=\"font-family:'");
            append_escaped_attr(&font, &mut self.out);
            self.out.push_str("'\">");
            self.out.push_str(&ent);
            self.out.push_str("</span>");
        } else {
            self.out.push_str(&ent);
        }
    }

    fn emit_note_ref(&mut self, xp: &mut XmlParser, footnote: bool, already_super: bool) {
        let id = xp.attr("w:id").to_string();
        xp.skip_element();
        if id.is_empty() {
            return;
        }
        let idx = {
            let order = if footnote { &mut self.fn_order } else { &mut self.en_order };
            match order.iter().position(|x| x == &id) {
                Some(i) => i,
                None => {
                    order.push(id.clone());
                    order.len() - 1
                }
            }
        };
        let prefix = if footnote { "fn" } else { "en" };
        let num = (idx + 1).to_string();
        if !already_super {
            self.out.push_str("<sup>");
        }
        self.out.push_str("<a href=\"#");
        self.out.push_str(prefix);
        self.out.push('-');
        self.out.push_str(&id);
        self.out.push_str("\" id=\"");
        self.out.push_str(prefix);
        self.out.push_str("ref-");
        self.out.push_str(&id);
        self.out.push_str("\">");
        self.out.push_str(&num);
        self.out.push_str("</a>");
        if !already_super {
            self.out.push_str("</sup>");
        }
    }

    fn append_notes_section(&mut self, part_name: &str, footnote: bool) {
        let order = if footnote { self.fn_order.clone() } else { self.en_order.clone() };
        if order.is_empty() {
            return;
        }
        let xml_buf = match self.load_part(&format!("{}{}.xml", self.base_dir, part_name)) {
            Some(b) => b,
            None => return,
        };
        // notes resolve hyperlinks/images through their own .rels part
        let mut doc_rels: HashMap<String, Relationship> = HashMap::new();
        std::mem::swap(&mut self.rels, &mut doc_rels);
        if let Some(rbuf) = self.load_part(&format!("{}_rels/{}.xml.rels", self.base_dir, part_name)) {
            let rs = String::from_utf8_lossy(&rbuf).into_owned();
            self.parse_rels(&rs);
        }

        let xml_str = String::from_utf8_lossy(&xml_buf).into_owned();
        let note_elem = if footnote { "w:footnote" } else { "w:endnote" };
        let mut body_by_id: HashMap<String, String> = HashMap::new();
        {
            let mut xp = XmlParser::new(&xml_str);
            loop {
                match xp.next() {
                    Event::Eof => break,
                    Event::Start if xp.name() == note_elem => {
                        let id = xp.attr("w:id").to_string();
                        let ty = xp.attr("w:type");
                        if matches!(ty, "separator" | "continuationSeparator" | "continuationNotice") {
                            xp.skip_element();
                            continue;
                        }
                        let saved = std::mem::take(&mut self.out);
                        self.parse_blocks(&mut xp, note_elem, 0);
                        self.close_lists(0);
                        body_by_id.insert(id, std::mem::replace(&mut self.out, saved));
                    }
                    _ => {}
                }
            }
        }
        std::mem::swap(&mut self.rels, &mut doc_rels);

        let prefix = if footnote { "fn" } else { "en" };
        self.out.push_str("<hr><section class=\"footnotes\"><ol>");
        for id in &order {
            self.out.push_str("<li id=\"");
            self.out.push_str(prefix);
            self.out.push('-');
            self.out.push_str(id);
            self.out.push_str("\">");
            if let Some(body) = body_by_id.get(id) {
                self.out.push_str(body);
            }
            self.out.push_str("<a href=\"#");
            self.out.push_str(prefix);
            self.out.push_str("ref-");
            self.out.push_str(id);
            self.out.push_str("\">&#8617;</a></li>");
        }
        self.out.push_str("</ol></section>");
    }

    // ---- drawings / images ----

    fn parse_drawing(&mut self, xp: &mut XmlParser, depth: i32) {
        let mut cx = 0i32;
        let mut cy = 0i32;
        let mut rel_id = String::new();
        let mut d = 1;
        while d > 0 {
            match xp.next() {
                Event::Eof => break,
                Event::Start => {
                    let n = xp.name();
                    if n == "wp:extent" {
                        cx = sv_to_int(xp.attr("cx"));
                        cy = sv_to_int(xp.attr("cy"));
                    } else if n == "a:blip" {
                        let mut r = xp.attr("r:embed");
                        if r.is_empty() {
                            r = xp.attr("r:link");
                        }
                        if !r.is_empty() {
                            rel_id = r.to_string();
                        }
                    } else if n == "v:imagedata" {
                        let r = xp.attr("r:id");
                        if !r.is_empty() {
                            rel_id = r.to_string();
                        }
                    } else if n == "w:txbxContent" {
                        let nm = n.to_string();
                        self.parse_blocks(xp, &nm, depth);
                        continue;
                    }
                    d += 1;
                }
                Event::End => d -= 1,
                _ => {}
            }
        }
        if !rel_id.is_empty() {
            self.emit_image(&rel_id, cx, cy);
        }
    }

    fn emit_image(&mut self, rel_id: &str, cx: i32, cy: i32) {
        if !self.opts.embed_images {
            return;
        }
        let (target, external) = match self.rels.get(rel_id) {
            Some(r) => (r.target.clone(), r.external),
            None => return,
        };
        let src;
        if external {
            src = target;
        } else {
            let path = resolve_zip_path(&self.base_dir, &target);
            let bytes = match self.zip.find(&path).and_then(|e| self.zip.extract(e)) {
                Some(b) => b,
                None => return,
            };
            src = format!("data:{};base64,{}", mime_for_path(&path), base64_encode(&bytes));
        }
        self.out.push_str("<img src=\"");
        append_escaped_attr(&src, &mut self.out);
        self.out.push_str("\" alt=\"\"");
        if cx > 0 && cy > 0 {
            self.out.push_str(&format!(" width=\"{}\"", cx / 9525));
            self.out.push_str(&format!(" height=\"{}\"", cy / 9525));
        }
        self.out.push('>');
    }

    // ---- tables ----

    fn parse_table(&mut self, xp: &mut XmlParser, depth: i32) {
        let mut rows: Vec<RowData> = Vec::new();
        self.collect_rows(xp, "w:tbl", &mut rows, depth);
        self.emit_table(&rows);
    }

    fn collect_rows(&mut self, xp: &mut XmlParser, end_name: &str, rows: &mut Vec<RowData>, depth: i32) {
        loop {
            match xp.next() {
                Event::Eof => return,
                Event::End if xp.name() == end_name => return,
                Event::Start => {}
                _ => continue,
            }
            let n = xp.name().to_string();
            if n == "w:tr" {
                rows.push(RowData::default());
                let last = rows.len() - 1;
                self.collect_row_children(xp, "w:tr", rows, last, depth);
            } else if n == "w:tblPr" || n == "w:tblGrid" || depth > MAX_DEPTH {
                xp.skip_element();
            } else {
                self.collect_rows(xp, &n, rows, depth + 1);
            }
        }
    }

    fn collect_row_children(&mut self, xp: &mut XmlParser, end_name: &str, rows: &mut Vec<RowData>, row_idx: usize, depth: i32) {
        loop {
            match xp.next() {
                Event::Eof => return,
                Event::End if xp.name() == end_name => return,
                Event::Start => {}
                _ => continue,
            }
            let n = xp.name().to_string();
            if n == "w:tc" {
                let mut cell = CellData { colspan: 1, ..Default::default() };
                self.collect_cell(xp, &mut cell, depth);
                rows[row_idx].cells.push(cell);
            } else if n == "w:trPr" {
                let mut d = 1;
                while d > 0 {
                    match xp.next() {
                        Event::Eof => return,
                        Event::Start => {
                            if xp.name() == "w:tblHeader" && toggle_on(xp.attr("w:val")) {
                                rows[row_idx].header = true;
                            }
                            d += 1;
                        }
                        Event::End => d -= 1,
                        _ => {}
                    }
                }
            } else if n == "w:tblPrEx" || depth > MAX_DEPTH {
                xp.skip_element();
            } else {
                self.collect_row_children(xp, &n, rows, row_idx, depth + 1);
            }
        }
    }

    fn collect_cell(&mut self, xp: &mut XmlParser, cell: &mut CellData, depth: i32) {
        let saved = std::mem::take(&mut self.out);
        loop {
            match xp.next() {
                Event::Eof => break,
                Event::End if xp.name() == "w:tc" => break,
                Event::Start => {
                    let n = xp.name().to_string();
                    if n == "w:tcPr" {
                        self.parse_cell_props(xp, cell);
                        continue;
                    }
                    self.parse_block_child(xp, &n, depth);
                }
                _ => {}
            }
        }
        self.close_lists(0);
        cell.html = std::mem::replace(&mut self.out, saved);
    }

    fn parse_cell_props(&mut self, xp: &mut XmlParser, cell: &mut CellData) {
        let mut depth = 1;
        while depth > 0 {
            match xp.next() {
                Event::Eof => return,
                Event::Start => {
                    let n = xp.name();
                    if n == "w:gridSpan" {
                        cell.colspan = sv_to_int(xp.attr("w:val")).max(1);
                    } else if n == "w:vMerge" {
                        cell.vmerge = if xp.attr("w:val") == "restart" { 1 } else { 2 };
                    } else if n == "w:shd" {
                        let fill = xp.attr("w:fill");
                        if !fill.is_empty() && fill != "auto" {
                            cell.css.push_str("background-color:#");
                            cell.css.push_str(fill);
                            cell.css.push(';');
                        }
                    } else if n == "w:vAlign" {
                        let v = xp.attr("w:val");
                        if v == "center" {
                            cell.css.push_str("vertical-align:middle;");
                        } else if v == "bottom" {
                            cell.css.push_str("vertical-align:bottom;");
                        }
                    }
                    depth += 1;
                }
                Event::End => depth -= 1,
                _ => {}
            }
        }
    }

    fn emit_table(&mut self, rows: &[RowData]) {
        let nr = rows.len();
        let mut start: Vec<Vec<i32>> = vec![Vec::new(); nr];
        let mut covered: Vec<Vec<bool>> = vec![Vec::new(); nr];
        let mut rowspan: Vec<Vec<i32>> = vec![Vec::new(); nr];
        for r in 0..nr {
            let mut c = 0;
            for cell in &rows[r].cells {
                start[r].push(c);
                c += cell.colspan;
            }
            covered[r] = vec![false; rows[r].cells.len()];
            rowspan[r] = vec![1; rows[r].cells.len()];
        }
        for r in 0..nr {
            for ci in 0..rows[r].cells.len() {
                if rows[r].cells[ci].vmerge != 1 || covered[r][ci] {
                    continue;
                }
                let col = start[r][ci];
                for rr in r + 1..nr {
                    let mut found = false;
                    for cj in 0..rows[rr].cells.len() {
                        if start[rr][cj] == col && rows[rr].cells[cj].vmerge == 2 && !covered[rr][cj] {
                            covered[rr][cj] = true;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        break;
                    }
                    rowspan[r][ci] += 1;
                }
            }
        }
        self.out.push_str("<table>");
        for r in 0..nr {
            self.out.push_str("<tr>");
            let tag = if rows[r].header { "th" } else { "td" };
            for ci in 0..rows[r].cells.len() {
                if covered[r][ci] {
                    continue;
                }
                let cell = &rows[r].cells[ci];
                self.out.push('<');
                self.out.push_str(tag);
                if cell.colspan > 1 {
                    self.out.push_str(&format!(" colspan=\"{}\"", cell.colspan));
                }
                if rowspan[r][ci] > 1 {
                    self.out.push_str(&format!(" rowspan=\"{}\"", rowspan[r][ci]));
                }
                if !cell.css.is_empty() {
                    self.out.push_str(" style=\"");
                    self.out.push_str(&cell.css);
                    self.out.push('"');
                }
                self.out.push('>');
                self.out.push_str(&cell.html);
                self.out.push_str("</");
                self.out.push_str(tag);
                self.out.push('>');
            }
            self.out.push_str("</tr>");
        }
        self.out.push_str("</table>");
    }

    // ---- top level ----

    fn run(&mut self) -> Result<String, String> {
        let doc_path = self.find_document_path();
        let doc_xml = match self.load_part(&doc_path) {
            Some(b) => b,
            None => {
                if self.zip.find("mimetype").is_some() && self.zip.find("content.xml").is_some() {
                    return Err("OpenDocument file (.odt) with a .docx extension (not supported)".into());
                }
                return Err(format!("not a valid .docx file (missing {doc_path})"));
            }
        };
        let slash = doc_path.rfind('/');
        self.base_dir = match slash {
            Some(s) => doc_path[..s + 1].to_string(),
            None => String::new(),
        };
        let doc_name = match slash {
            Some(s) => doc_path[s + 1..].to_string(),
            None => doc_path.clone(),
        };

        if let Some(buf) = self.load_part(&format!("{}_rels/{}.rels", self.base_dir, doc_name)) {
            let s = String::from_utf8_lossy(&buf).into_owned();
            self.parse_rels(&s);
        }
        if let Some(buf) = self.load_part(&format!("{}styles.xml", self.base_dir)) {
            let s = String::from_utf8_lossy(&buf).into_owned();
            self.parse_styles(&s);
        }
        if let Some(buf) = self.load_part(&format!("{}numbering.xml", self.base_dir)) {
            let s = String::from_utf8_lossy(&buf).into_owned();
            self.parse_numbering(&s);
        }
        if let Some(buf) = self.load_part("docProps/core.xml") {
            let s = String::from_utf8_lossy(&buf).into_owned();
            self.parse_core_props(&s);
        }

        let doc_str = String::from_utf8_lossy(&doc_xml).into_owned();
        self.out.reserve(doc_xml.len());
        self.convert_body(&doc_str);
        self.append_notes_section("footnotes", true);
        self.append_notes_section("endnotes", false);

        if self.opts.fragment {
            return Ok(std::mem::take(&mut self.out));
        }
        let title = if self.opts.title.is_empty() { self.title.clone() } else { self.opts.title.clone() };
        let mut html = String::with_capacity(self.out.len() + 1024);
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
        append_escaped_html(if title.is_empty() { "Document" } else { &title }, &mut html);
        html.push_str(
            "</title>\n<style>\n\
             body{font-family:Calibri,Arial,sans-serif;line-height:1.4;max-width:60em;margin:2em auto;padding:0 1em;}\n\
             table{border-collapse:collapse;margin:0.5em 0;}\n\
             td,th{border:1px solid #999;padding:0.25em 0.5em;vertical-align:top;}\n\
             th{background:#f0f0f0;text-align:left;}\n\
             img{max-width:100%;height:auto;}\n\
             .tab{white-space:pre;}\n\
             .footnotes{font-size:0.9em;color:#444;}\n\
             </style>\n</head>\n<body>\n",
        );
        html.push_str(&self.out);
        html.push_str("\n</body>\n</html>\n");
        Ok(html)
    }
}

fn build_run_tags(rp: &RunProps, open: &mut String, close: &mut String) {
    let mut wrap = |o: &str, c: &str, open: &mut String, close: &mut String| {
        open.push_str(o);
        close.insert_str(0, c);
    };
    if rp.b {
        wrap("<strong>", "</strong>", open, close);
    }
    if rp.i {
        wrap("<em>", "</em>", open, close);
    }
    if rp.u {
        wrap("<u>", "</u>", open, close);
    }
    if rp.strike {
        wrap("<s>", "</s>", open, close);
    }
    if rp.vert == 1 {
        wrap("<sup>", "</sup>", open, close);
    }
    if rp.vert == -1 {
        wrap("<sub>", "</sub>", open, close);
    }
    let mut css = String::new();
    if !rp.color.is_empty() {
        css.push_str(&format!("color:#{};", rp.color));
    }
    if !rp.highlight.is_empty() {
        css.push_str(&format!("background-color:{};", highlight_css(&rp.highlight)));
    }
    if rp.caps {
        css.push_str("text-transform:uppercase;");
    }
    if rp.small_caps {
        css.push_str("font-variant:small-caps;");
    }
    if rp.vanish {
        css.push_str("display:none;");
    }
    if !css.is_empty() {
        open.push_str(&format!("<span style=\"{css}\">"));
        close.insert_str(0, "</span>");
    }
}
