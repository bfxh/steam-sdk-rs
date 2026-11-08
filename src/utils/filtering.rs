//! Parental/text filtering: initialising it, asking Steam to filter a string, what context
//! that string is judged in, and the buffer copy that turns Steam's answer into a
//! `FilteredText`.

use super::*;

/// Which kind of content a filtered string is, as Steam classifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFilteringContext {
    Unknown,
    GameContent,
    Chat,
    Name,
}

impl From<TextFilteringContext> for sys::ETextFilteringContext {
    fn from(context: TextFilteringContext) -> Self {
        match context {
            TextFilteringContext::Unknown => {
                sys::ETextFilteringContext::k_ETextFilteringContextUnknown
            }
            TextFilteringContext::GameContent => {
                sys::ETextFilteringContext::k_ETextFilteringContextGameContent
            }
            TextFilteringContext::Chat => sys::ETextFilteringContext::k_ETextFilteringContextChat,
            TextFilteringContext::Name => sys::ETextFilteringContext::k_ETextFilteringContextName,
        }
    }
}

/// What [`Utils::filter_text`] produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilteredText {
    /// The text Steam wrote back, which equals the input when nothing was filtered.
    pub text: String,
    /// Characters Steam changed -- a count of characters, not bytes.
    pub filtered_characters: usize,
}

/// Reads the filter output without trusting it to be terminated: the buffer is
/// cut at the first nul, and a negative return from Steam is a failure rather
/// than zero changes.
pub(super) fn filtered_text_from_buffer(
    buffer: &[std::os::raw::c_char],
    filtered: i32,
) -> Result<FilteredText, SteamError> {
    if filtered < 0 {
        return Err(SteamError::IOFailure);
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .map(|byte| *byte as u8)
        .take_while(|byte| *byte != 0)
        .collect();
    Ok(FilteredText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        filtered_characters: filtered as usize,
    })
}

impl Utils {
    /// Loads the text-filter dictionaries for the language Steam is running in.
    ///
    /// `options` is reserved by Steam and must be 0. Returns whether filtering is
    /// available: when it returns false, Steam says
    /// [`Utils::filter_text`] acts as a passthrough rather than failing, so a
    /// caller must not read "unavailable" as "clean text".
    pub fn init_text_filter(&self, options: u32) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_InitFilterText(self.utils, options) }
    }

    /// Runs Steam's community text filter over `input`.
    ///
    /// `source` is who the text belongs to (the chat speaker, or the owner of a
    /// player name), because the user's own filter settings apply per source.
    /// Steam writes the result -- filtered or not -- into a buffer it expects to
    /// be at least one byte larger than the input, which this allocates for you,
    /// and returns how many characters it changed.
    pub fn filter_text(
        &self,
        context: TextFilteringContext,
        source: SteamId,
        input: &str,
    ) -> Result<FilteredText, SteamError> {
        let input = CString::new(input).map_err(|_| SteamError::InvalidParameter)?;
        let input_len = input.as_bytes().len();
        // Steam asks for strlen(input)+1; the extra slot is the terminator this
        // reads back to, and it is pre-zeroed so a missing terminator cannot
        // make the read run past the allocation.
        let mut out: Vec<std::os::raw::c_char> = vec![0; input_len + 1];
        let filtered = unsafe {
            sys::SteamAPI_ISteamUtils_FilterText(
                self.utils,
                context.into(),
                source.raw(),
                input.as_ptr(),
                out.as_mut_ptr(),
                u32::try_from(out.len()).map_err(|_| SteamError::InvalidParameter)?,
            )
        };
        filtered_text_from_buffer(&out, filtered)
    }
}
