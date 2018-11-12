use std::cell::RefCell;
use std::rc::Rc;
pub use font_kit::family_name::FamilyName;
pub use font_kit::properties::Properties;
pub use font_kit::hinting::HintingOptions;
pub use font_kit::error::GlyphLoadingError;
use lyon_path::builder::PathBuilder;

#[derive(Clone)]
pub struct Fonts {
    container: Rc<RefCell<shared::FontsContainer>>,
}

impl Fonts {
    pub fn new() -> Fonts {
        Fonts {
            container: Rc::new(RefCell::new(shared::FontsContainer::new())),
        }
    }

    pub fn find_best_match(&self, family_names: &[FamilyName], properties: &Properties) -> Option<Font> {
        let mut shared = self.container.borrow_mut();

        shared.find_best_match(family_names, properties)
            .map(|id| Font {
                id,
                container: self.container.clone()
            })
    }

    pub fn font_from_id(&self, id: usize) -> Option<Font> {
        let mut shared = self.container.borrow_mut();

        Some(Font {
            container: self.container.clone(),
            id: shared.get_and_inc_font(id)?,
        })
    }

    pub fn buffer_from_id(&self, buffer_id: usize) -> Option<Buffer> {
        let mut shared = self.container.borrow_mut();

        let (font_id, buffer_id) = shared.get_and_inc_buffer(buffer_id)?;

        Some(Buffer {
            _font: Font {
                container: self.container.clone(),
                id: shared.get_and_inc_font(font_id)?,
            },
            id: buffer_id,
        })
    }

    pub fn glyphs(&self, buffer: BufferRef) -> () {

    }
}

#[derive(Clone)]
pub struct Glyph {
    id: u32,
}

pub struct Font {
    id: usize,
    container: Rc<RefCell<shared::FontsContainer>>,
}

impl Font {
    pub fn full_name(&self) -> String {
        let shared = self.container.borrow();
        shared.get(self.id)
            .expect("full_name: loaded font should exist")
            .fk_font.full_name()
    }

    pub fn glyph_count(&self) -> u32 {
        let shared = self.container.borrow();
        shared.get(self.id)
            .expect("glyph_count: loaded font should exist")
            .fk_font.glyph_count()
    }

    pub fn outline<B>(&self, glyph_id: u32, hinting: HintingOptions, path_builder: &mut B)
                      -> Result<(), GlyphLoadingError>
        where B: PathBuilder {
        let shared = self.container.borrow();
        shared.get(self.id)
            .expect("outline: loaded font should exist")
            .fk_font.outline(glyph_id, hinting, path_builder)
    }

    pub fn create_buffer<P: ToString>(&self, text: P) -> Buffer {
        Buffer::new(self.clone(), text)
    }
}

impl Clone for Font {
    fn clone(&self) -> Self {
        let mut shared = self.container.borrow_mut();
        shared.inc_font(self.id);
        Font {
            id: self.id,
            container: self.container.clone(),
        }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        let mut shared = self.container.borrow_mut();
        shared.dec_font(self.id);
    }
}

pub struct Buffer {
    _font: Font,
    id: usize,
}

impl Buffer {
    fn new<P: ToString>(font: Font, text: P) -> Buffer {
        let id = {
            let mut shared = font.container.borrow_mut();
            shared.create_buffer(font.id, text)
        };

        Buffer {
            _font: font,
            id,
        }
    }

    pub fn weak_ref(&self) -> BufferRef {
        BufferRef {
            font_id: self._font.id,
            id: self.id,
        }
    }

    pub fn font(&self) -> &Font {
        &self._font
    }

    pub fn glyph_ids(&self, output: &mut Vec<u32>) {
        let shared = self._font.container.borrow();
        shared.buffer_glyph_ids(self.id, output)
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        let mut shared = self._font.container.borrow_mut();
        shared.inc_buffer(self.id);
        shared.inc_font(self._font.id);

        Buffer {
            id: self.id,
            _font: Font {
                id: self._font.id,
                container: self._font.container.clone(),
            },
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let mut shared = self._font.container.borrow_mut();
        shared.dec_buffer(self.id)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct BufferRef {
    pub font_id: usize,
    pub id: usize,
}

mod shared {
    use harfbuzz_rs as hb;

    use slab::Slab;
    use metrohash::MetroHashMap;
    use int_hash::IntHashMap;
    use sha1::{Digest, Sha1};

    use font_kit::source::SystemSource;
    use font_kit::family_name::FamilyName;
    use font_kit::properties::Properties;
    use font_kit::handle::Handle;
    use font_kit::font::Font as FontkitFont;
    use byteorder::{LittleEndian, WriteBytesExt};

    pub struct BufferData {
        text: String,
        buffer: Option<hb::GlyphBuffer>,
        font_id: usize,
        count: usize,
    }

    impl BufferData {
        fn new<P: ToString>(font_id: usize, font_data: &FontData, text: P) -> BufferData {
            let text = text.to_string();
            let unicode_buffer = hb::UnicodeBuffer::new().add_str(&text);

            let buffer = Some({
                let font = &font_data.hb_font;

                hb::shape(&font, unicode_buffer, &[])
            });

            BufferData {
                text,
                buffer,
                font_id,
                count: 1,
            }
        }

        fn replace(&mut self, font_data: &FontData, text: &str) {
            self.text.clear();
            self.text.push_str(text);
            self.shape(font_data)
        }

        fn shape(&mut self, font_data: &FontData) {
            let font = &font_data.hb_font;

            let mut unicode_buffer = ::std::mem::replace(&mut self.buffer, None).unwrap().clear();
            unicode_buffer = unicode_buffer.add_str(&self.text);

            ::std::mem::replace(&mut self.buffer, Some(hb::shape(&font, unicode_buffer, &[])));

//        // The results of the shaping operation are stored in the `output` buffer.
//
//        let positions = output.get_glyph_positions();
//        let infos = output.get_glyph_infos();
//
//        // iterate over the shaped glyphs
//        for (position, info) in positions.iter().zip(infos) {
//            let gid = info.codepoint;
//            let cluster = info.cluster;
//            let x_advance = position.x_advance;
//            let x_offset = position.x_offset;
//            let y_offset = position.y_offset;
//
//            // Here you would usually draw the glyphs.
//            println!("gid{:?}={:?}@{:?},{:?}+{:?}", gid, cluster, x_advance, x_offset, y_offset);
//        }
        }

        fn glyph_ids(&self, output: &mut Vec<u32>) {
            output.extend(self.buffer.as_ref().unwrap()
                .get_glyph_infos().iter().map(|i| i.codepoint));
        }
    }

    pub struct FontData {
        pub fk_font: FontkitFont,
        pub hb_font: hb::Owned<hb::Font<'static>>,
        pub count: usize,
    }

    pub struct FontsContainer {
        system_source: SystemSource,

        fonts: Slab<[u8; 20]>,
        fonts_fingerprint_id: MetroHashMap<[u8; 20], usize>,
        fonts_id_prop: IntHashMap<usize, FontData>,

        buffers: Slab<BufferData>,
    }

    impl FontsContainer {
        pub fn new() -> FontsContainer {
            FontsContainer {
                system_source: SystemSource::new(),

                fonts: Slab::new(),
                fonts_fingerprint_id: MetroHashMap::default(),
                fonts_id_prop: IntHashMap::default(),

                buffers: Slab::new(),
            }
        }

        pub fn create_buffer<P: ToString>(&mut self, font_id: usize, text: P) -> usize {
            let buffer = {
                let font_data = self.get(font_id).expect("FontsContainer::create_buffer - self.get(font_id)");
                BufferData::new(font_id, font_data, text)
            };

            self.buffers.insert(buffer)
        }

        pub fn buffer_glyph_ids(&self, buffer_id: usize, output: &mut Vec<u32>) {
            self.buffers.get(buffer_id).expect("buffer_glyph_ids: self.buffers.get(buffer_id)")
                .glyph_ids(output)
        }

        pub fn get_and_inc_buffer(&mut self, id: usize) -> Option<(usize, usize)> {
            let buffer_data = self.buffers.get_mut(id)?;
            buffer_data.count += 1;
            Some((buffer_data.font_id, id))
        }

        pub fn inc_buffer(&mut self, id: usize) {
            let data = self.buffers.get_mut(id).expect("inc_buffer: self.buffers.get_mut(id)");
            data.count += 1;
        }

        pub fn dec_buffer(&mut self, id: usize) {
            let delete = {
                let data = self.buffers.get_mut(id).expect("dec_buffer: self.buffers.get_mut(id)");
                data.count -= 1;
                data.count <= 0
            };

            if delete {
                self.delete_buffer(id);
            }
        }

        pub fn delete_buffer(&mut self, id: usize) {
            self.buffers.remove(id);
        }

        pub fn inc_font(&mut self, id: usize) {
            let data = self.fonts_id_prop.get_mut(&id).expect("inc_font: self.fonts_id_prop.get_mut(&id)");
            data.count += 1;
        }

        pub fn get_and_inc_font(&mut self, id: usize) -> Option<usize> {
            let data = self.fonts_id_prop.get_mut(&id)?;
            data.count += 1;
            Some(id)
        }

        pub fn dec_font(&mut self, id: usize) {
            let delete = {
                let data = self.fonts_id_prop.get_mut(&id).expect("dec_font: self.fonts.get_mut(id)");
                data.count -= 1;
                data.count <= 0
            };

            if delete {
                self.delete_font(id);
            }
        }

        pub fn find_best_match(&mut self, family_names: &[FamilyName], properties: &Properties) -> Option<usize> {
            let font_handle = match self.system_source.select_best_match(family_names, properties) {
                Ok(handle) => handle,
                Err(_) => return None,
            };

            let fingerprint = generate_fingerprint(&font_handle);

            let mut id = self.fonts_fingerprint_id.get(&fingerprint).map(|v| *v);

            match id {
                None => {
                    match font_handle.load() {
                        Err(e) => {
                            error!("failed to load font: {:?}", e);
                            return None;
                        },
                        Ok(fk_font) => {
                            let face = match font_handle {
                                Handle::Path { path, font_index } => {
                                    match hb::Face::from_file(&path, font_index) {
                                        Err(e) => {
                                            error!("failed to load font face from {:?} - {:?}: {:?}", path, font_index, e);
                                            return None;
                                        },
                                        Ok(f) => f,
                                    }
                                },
                                Handle::Memory { .. } => unimplemented!("can not load fonts from memory"),
                            };

                            let mut hb_font = hb::Font::new(face);

                            use harfbuzz_rs::rusttype::SetRustTypeFuncs;
                            if let Err(e) = hb_font.set_rusttype_funcs() {
                                error!("failed to set up rusttype: {:?}", e);
                                return None;
                            }

                            let new_id = self.fonts.insert(fingerprint.clone());
                            id = Some(new_id);

                            debug!("load font {:?}", fk_font.full_name());

                            let data = FontData {
                                fk_font,
                                hb_font,
                                count: 1,
                            };

                            self.fonts_fingerprint_id.insert(fingerprint, new_id);
                            self.fonts_id_prop.insert(new_id, data);
                        }
                    };
                }
                Some(id) => {
                    self.inc_font(id);
                }
            }

            return id;
        }

        pub fn delete_font(&mut self, id: usize) {
            debug!("unload font {:?}", self.fonts_id_prop[&id].fk_font.full_name());

            self.fonts_id_prop.remove(&id);
            let fingerprint = self.fonts.remove(id);
            self.fonts_fingerprint_id.remove(&fingerprint);
        }

        pub fn get(&self, id: usize) -> Option<&FontData> {
            self.fonts_id_prop.get(&id)
        }
    }

    fn generate_fingerprint(handle: &Handle) -> [u8; 20] {
        let generic_array = match *handle {
            Handle::Path { ref path, font_index } => {
                let mut hasher = Sha1::new();
                hasher.input(path.to_string_lossy().as_bytes());

                let mut bytes = [0u8; 4];
                {
                    let mut cursor = ::std::io::Cursor::new(&mut bytes[..]);
                    cursor.write_u32::<LittleEndian>(font_index).unwrap();
                }
                hasher.input(&bytes);

                hasher.result()
            },
            Handle::Memory { ref bytes, font_index } => {
                let mut hasher = Sha1::new();
                hasher.input(&**bytes);

                let mut bytes = [0u8; 4];
                {
                    let mut cursor = ::std::io::Cursor::new(&mut bytes[..]);
                    cursor.write_u32::<LittleEndian>(font_index).unwrap();
                }
                hasher.input(&bytes);

                hasher.result()
            },
        };

        let mut output = [0; 20];

        for (input, output) in generic_array.iter().zip(output.iter_mut()) {
            *output = *input;
        }

        output
    }
}