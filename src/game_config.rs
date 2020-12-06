use super::{alphabet,board_layout};

pub trait GameConfig<'a> {
fn alphabet(&self)->&'a dyn alphabet::Alphabet<'a>;
fn board_layout(&self)->Box<dyn board_layout::BoardLayout<'a>>;
}

pub struct GenericGameConfig<'a> {
  alphabet : &'a (dyn alphabet::Alphabet<'a>+Sync),
  //bl :&'a (dyn board_layout::BoardLayout<'a>+Sync),
  bl : Box<dyn board_layout::BoardLayout<'a>>,
}

impl<'a> GameConfig<'a> for GenericGameConfig<'a> {
fn alphabet(&self)->&'a dyn alphabet::Alphabet<'a>{self.alphabet}
fn board_layout(&self)->Box<dyn board_layout::BoardLayout<'a>>{self.bl}
}

pub static COMMON_ENGLISH_GAME_CONFIG : GenericGameConfig = GenericGameConfig{
  alphabet:  &alphabet::ENGLISH_ALPHABET,
  bl:  board_layout::COMMON_BOARD_LAYOUT,
};
