macro_rules! define_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

define_id!(BodyId);
define_id!(BasicBlockId);
define_id!(StatementId);
define_id!(LocalId);
define_id!(ScopeId);
define_id!(CleanupId);
define_id!(ClosureId);
