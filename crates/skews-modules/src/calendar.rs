//! Calendar module: current date on the bar and a month grid panel.
//!
//! ```toml
//! [modules.calendar]
//! # no options yet
//! ```
//!
//! Interactions: click toggles the month panel; today's row is highlighted.

use chrono::{Datelike, Duration, Local, NaiveDate};
use skews_app::{ListItem, Module, ModuleOutput, Msg, PanelContent, Registry};
use skews_core::{Effect, Effects, ModuleId};

/// Shows the date and a month grid.
pub struct Calendar {
    id: ModuleId,
    last_text: Option<String>,
}

impl Calendar {
    /// Builds the module from its configuration options.
    pub fn new(_options: &toml::Value) -> Result<Self, String> {
        Ok(Self {
            id: ModuleId::from("calendar"),
            last_text: None,
        })
    }

    fn apply(&mut self) -> Effects {
        let text = Self::text_for(Local::now().date_naive());
        if self.last_text.as_deref() == Some(text.as_str()) {
            return Vec::new();
        }
        self.last_text = Some(text);
        vec![Effect::Redraw]
    }

    /// Bar text for a date (`%a %d`).
    fn text_for(date: NaiveDate) -> String {
        date.format("%a %d").to_string()
    }

    /// Monday-first weeks covering the month of `today`, padded with
    /// neighbouring days so every row has seven entries.
    fn weeks(today: NaiveDate) -> Vec<Vec<NaiveDate>> {
        let first = today.with_day(1).unwrap_or(today);
        let mut weeks = Vec::new();
        let mut week: Vec<NaiveDate> = Vec::new();

        let offset = first.weekday().num_days_from_monday();
        for back in (1..=offset).rev() {
            week.push(first - Duration::days(i64::from(back)));
        }

        let mut day = first;
        while day.month() == first.month() {
            week.push(day);
            if week.len() == 7 {
                weeks.push(std::mem::take(&mut week));
            }
            match day.succ_opt() {
                Some(next) => day = next,
                None => break,
            }
        }

        if week.is_empty() {
            return weeks;
        }

        while week.len() < 7 {
            week.push(day);
            match day.succ_opt() {
                Some(next) => day = next,
                None => break,
            }
        }
        weeks.push(week);

        weeks
    }
}

impl Module for Calendar {
    fn id(&self) -> &ModuleId {
        &self.id
    }

    fn update(&mut self, msg: &Msg) -> Effects {
        match msg {
            Msg::Tick { .. } => self.apply(),
            Msg::Interaction {
                module,
                kind: skews_core::InteractionKind::Click,
            } if module == &self.id => vec![Effect::Action(skews_core::Action::TogglePanel(
                self.id.clone(),
            ))],
            _ => Vec::new(),
        }
    }

    fn output(&self) -> ModuleOutput {
        match &self.last_text {
            Some(text) => ModuleOutput::Text(text.clone()),
            None => ModuleOutput::Empty,
        }
    }

    fn panel(&self) -> Option<PanelContent> {
        let today = Local::now().date_naive();
        let weeks = Self::weeks(today);

        let mut items = vec![ListItem {
            label: String::from("Mo Tu We Th Fr Sa Su"),
            detail: String::new(),
            active: false,
        }];
        items.extend(weeks.iter().map(|week| {
            ListItem {
                label: week
                    .iter()
                    .map(|day| format!("{:>2}", day.day()))
                    .collect::<Vec<_>>()
                    .join(" "),
                detail: String::new(),
                active: week.contains(&today),
            }
        }));

        Some(PanelContent::List {
            title: today.format("%B %Y").to_string(),
            items,
        })
    }
}

/// Registers the calendar module.
pub fn register(registry: &mut Registry) {
    registry.register("calendar", |options| {
        Calendar::new(options).map(|module| Box::new(module) as Box<dyn Module>)
    });
}

#[cfg(test)]
mod tests {
    use super::Calendar;
    use chrono::NaiveDate;
    use skews_app::{Module, ModuleOutput, Msg, PanelContent};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn module() -> Calendar {
        Calendar::new(&toml::Value::Table(toml::Table::new())).unwrap()
    }

    #[test]
    fn weeks_are_monday_first_and_seven_days_wide() {
        // 2026-09-01 is a Tuesday: the first week starts on Monday 08-31.
        let weeks = Calendar::weeks(date(2026, 9, 1));

        assert_eq!(weeks.len(), 5);
        for week in &weeks {
            assert_eq!(week.len(), 7);
        }
        assert_eq!(weeks[0][0], date(2026, 8, 31));
        assert_eq!(weeks[0][1], date(2026, 9, 1));
        assert!(weeks.iter().flatten().any(|day| *day == date(2026, 9, 26)));
        assert_eq!(weeks[4][6], date(2026, 10, 4));
    }

    #[test]
    fn month_starting_on_monday_has_no_leading_padding() {
        // 2026-06-01 is a Monday.
        let weeks = Calendar::weeks(date(2026, 6, 1));
        assert_eq!(weeks[0][0], date(2026, 6, 1));
    }

    #[test]
    fn bar_text_formats_the_weekday_and_day() {
        assert_eq!(Calendar::text_for(date(2026, 9, 26)), "Sat 26");
    }

    #[test]
    fn panel_highlights_today_row() {
        let mut module = module();
        module.update(&Msg::Tick { unix_ms: 0 });
        let today = chrono::Local::now().date_naive();

        let Some(PanelContent::List { items, title }) = module.panel() else {
            panic!("expected a list panel");
        };

        assert_eq!(title, today.format("%B %Y").to_string());
        assert_eq!(items.len(), Calendar::weeks(today).len() + 1);
        assert!(items.iter().any(|item| item.active));
        assert_eq!(
            module.output(),
            ModuleOutput::Text(Calendar::text_for(today))
        );
    }

    #[test]
    fn tick_is_idempotent() {
        let mut module = module();
        module.update(&Msg::Tick { unix_ms: 0 });
        assert_eq!(
            module.update(&Msg::Tick { unix_ms: 0 }),
            Vec::<skews_core::Effect>::new()
        );
    }
}
