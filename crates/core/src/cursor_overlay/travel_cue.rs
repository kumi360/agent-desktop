/// How the cursor presents a travel toward an action's target: whether it
/// arrives showing the pointer image, and what it says it is about to do.
/// The narration is shown before the cursor sets off, so the step is
/// announced before it happens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TravelCue<'a> {
    pub(crate) pointer: bool,
    pub(crate) narration: Option<&'a str>,
}

impl<'a> TravelCue<'a> {
    pub(crate) const fn new(pointer: bool, narration: Option<&'a str>) -> Self {
        Self { pointer, narration }
    }
}
