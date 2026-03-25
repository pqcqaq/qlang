macro_rules! define_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(u32);

        impl $name {
            pub(crate) const fn from_index(index: usize) -> Self {
                Self(index as u32)
            }

            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

define_id!(ItemId);
define_id!(TypeId);
define_id!(BlockId);
define_id!(StmtId);
define_id!(PatternId);
define_id!(ExprId);
define_id!(LocalId);
