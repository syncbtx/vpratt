pub trait VprattToken {
    type Item;
    type PrattToken: Clone + Copy + PartialEq;
    fn extract(item: &Self::Item) -> Self::PrattToken;
}

pub trait Spanned {
    type Span: Copy + Into<core::ops::Range<usize>>;
    fn span(&self) -> Self::Span;
}