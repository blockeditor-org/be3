use be_sdk_beui::be_block::ObjectId;
use be_sdk_test::BeuiTest;
use be_type_example_scoreboard::{Scoreboard as ScoreboardModel, ScoreboardContent};
use uuid::Uuid;

use crate::root::ScoreboardRoot;

mod adding_a_player_puts_them_on_the_board;
mod resetting_puts_every_score_back_to_zero;
mod scoring_moves_a_player_up_the_standings_in_place;

struct Harness {
    editor: BeuiTest<ScoreboardRoot>,
}

impl Harness {
    fn new(players: &[(&str, i64)]) -> Self {
        let mut content = ScoreboardContent::default();
        for (name, score) in players {
            let (id, add) = ScoreboardModel::add(*name);
            content.apply(&add);
            content.apply(&ScoreboardModel::score(id, *score));
        }
        let mut harness = Self {
            editor: BeuiTest::editing(Uuid::new_v4()),
        };
        harness
            .editor
            .store()
            .register(be_type_example_scoreboard::registrations());
        harness.editor.hold(None, content);
        harness.editor.run();
        harness
    }

    fn click(&mut self, test_id: &str) {
        self.editor.click(test_id);
        self.editor.run();
    }

    fn type_text(&mut self, text: &str) {
        self.editor.type_text(text);
        self.editor.run();
    }

    fn content(&self) -> ScoreboardContent {
        self.editor.content(None)
    }

    fn id(&self, name: &str) -> ObjectId {
        self.content()
            .root()
            .players
            .iter()
            .find(|player| player.name == name)
            .map(|player| player.id)
            .expect("no such player")
    }

    fn scores(&self) -> Vec<(String, i64)> {
        self.content()
            .root()
            .players
            .iter()
            .map(|player| (player.name.clone(), player.score.get()))
            .collect()
    }

    fn shown(&self, test_id: &str) -> String {
        let document = self.editor.document();
        let node = document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("nothing is marked {test_id}"));
        document.node_detail(node).expect("the node has no text")
    }
}
