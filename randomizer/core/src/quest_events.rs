use crate::medal_gate::Goal;

pub const PREFIX: &str = "OOTRR_Event_";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ev {
    pub key: &'static str,
    pub title: &'static str,
    pub after: &'static str,
    pub goal: Goal,
}

pub fn plan(p: &crate::slice::support::Plan) -> Vec<Ev> {
    let mut v: Vec<Ev> = Vec::new();
    if let Some(g) = p.lacs.filter(|g| g.n() > 0) {
        v.push(Ev { key: "Lacs", title: "Ganon's Key", after: "Visit Sheik in the Temple of Time.", goal: g });
    }
    if let Some(g) = p.rainbow.filter(|g| g.n() > 0) {
        v.push(Ev { key: "Bridge", title: "Rainbow Bridge", after: "The rainbow bridge is now open.", goal: g });
    }
    v
}

impl Ev {
    pub fn id(&self) -> String {
        format!("{PREFIX}{}", self.key)
    }

    pub fn seen(&self) -> String {
        format!("OOTRREventSeen_{}", self.key)
    }

    pub fn line(&self) -> String {
        let n: u8 = self.goal.n();
        let things: &str = match (self.goal, n) {
            (Goal::Rewards(_), 1) => "Stone or Medallion",
            (Goal::Rewards(_), _) => "Stones and Medallions",
            (Goal::Medallions(_), 1) => "Medallion",
            (Goal::Medallions(_), _) => "Medallions",
        };
        format!("You hold {n} {things}.\n\n{}", self.after)
    }
}
