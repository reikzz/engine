use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfileRecord {
    pub id: &'static str,
    pub evaluations: u64,
    pub invocations: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
}

impl SystemProfileRecord {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            evaluations: 0,
            invocations: 0,
            total_duration: Duration::ZERO,
            min_duration: Duration::MAX,
            max_duration: Duration::ZERO,
        }
    }

    pub fn record_run(&mut self, duration: Duration) {
        self.evaluations = self.evaluations.saturating_add(1);
        self.invocations = self.invocations.saturating_add(1);
        self.total_duration = self.total_duration.saturating_add(duration);
        if duration < self.min_duration {
            self.min_duration = duration;
        }
        if duration > self.max_duration {
            self.max_duration = duration;
        }
    }

    pub fn record_skip(&mut self) {
        self.evaluations = self.evaluations.saturating_add(1);
    }

    pub fn average_duration(&self) -> Duration {
        if self.invocations == 0 {
            Duration::ZERO
        } else {
            self.total_duration / (self.invocations as u32)
        }
    }

    pub fn min_duration(&self) -> Duration {
        if self.invocations == 0 {
            Duration::ZERO
        } else {
            self.min_duration
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SystemProfiler {
    records: Vec<SystemProfileRecord>,
    total_ticks: u64,
    pipeline_ticks: u64,
    commands_duration: Duration,
    advance_duration: Duration,
}

impl SystemProfiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_system(&mut self, id: &'static str) {
        self.records.push(SystemProfileRecord::new(id));
    }

    pub fn records(&self) -> &[SystemProfileRecord] {
        &self.records
    }

    pub fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    pub fn pipeline_ticks(&self) -> u64 {
        self.pipeline_ticks
    }

    pub fn commands_duration(&self) -> Duration {
        self.commands_duration
    }

    pub fn advance_duration(&self) -> Duration {
        self.advance_duration
    }

    pub fn total_systems_duration(&self) -> Duration {
        self.records.iter().map(|r| r.total_duration).sum()
    }

    pub fn record_advance(&mut self, duration: Duration) {
        self.total_ticks = self.total_ticks.saturating_add(1);
        self.advance_duration = self.advance_duration.saturating_add(duration);
    }

    pub fn record_pipeline_tick(&mut self) {
        self.pipeline_ticks = self.pipeline_ticks.saturating_add(1);
    }

    pub fn record_commands(&mut self, duration: Duration) {
        self.commands_duration = self.commands_duration.saturating_add(duration);
    }

    pub fn record_system_run(&mut self, index: usize, id: &'static str, duration: Duration) {
        self.ensure_capacity(index, id);
        self.records[index].record_run(duration);
    }

    pub fn record_system_skip(&mut self, index: usize, id: &'static str) {
        self.ensure_capacity(index, id);
        self.records[index].record_skip();
    }

    fn ensure_capacity(&mut self, index: usize, id: &'static str) {
        if index >= self.records.len() {
            self.records
                .resize_with(index + 1, || SystemProfileRecord::new(""));
        }
        if self.records[index].id.is_empty() {
            self.records[index].id = id;
        }
    }

    pub fn build_report(&self, wall_duration: Duration) -> SystemProfileReport {
        let total_systems = self.total_systems_duration();
        let total_systems_secs = total_systems.as_secs_f64().max(1e-9);
        let wall_secs = wall_duration.as_secs_f64().max(1e-9);

        let mut items: Vec<ProfileReportItem> = self
            .records
            .iter()
            .map(|r| {
                let r_secs = r.total_duration.as_secs_f64();
                let share_of_systems = (r_secs / total_systems_secs) * 100.0;
                let share_of_wall = (r_secs / wall_secs) * 100.0;
                ProfileReportItem {
                    id: r.id,
                    evaluations: r.evaluations,
                    invocations: r.invocations,
                    total_duration: r.total_duration,
                    avg_duration: r.average_duration(),
                    min_duration: r.min_duration(),
                    max_duration: r.max_duration,
                    percentage_of_systems: share_of_systems.clamp(0.0, 100.0),
                    percentage_of_total: share_of_wall.clamp(0.0, 100.0),
                }
            })
            .collect();

        items.sort_by(|a, b| b.total_duration.cmp(&a.total_duration));

        SystemProfileReport {
            items,
            total_ticks: self.total_ticks,
            pipeline_ticks: self.pipeline_ticks,
            total_systems_duration: total_systems,
            commands_duration: self.commands_duration,
            advance_duration: self.advance_duration,
            wall_duration,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileReportItem {
    pub id: &'static str,
    pub evaluations: u64,
    pub invocations: u64,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub percentage_of_systems: f64,
    pub percentage_of_total: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemProfileReport {
    pub items: Vec<ProfileReportItem>,
    pub total_ticks: u64,
    pub pipeline_ticks: u64,
    pub total_systems_duration: Duration,
    pub commands_duration: Duration,
    pub advance_duration: Duration,
    pub wall_duration: Duration,
}

impl SystemProfileReport {
    pub fn top_bottlenecks(&self, count: usize) -> &[ProfileReportItem] {
        let len = self.items.len().min(count);
        &self.items[..len]
    }

    pub fn accounted_duration(&self) -> Duration {
        self.total_systems_duration
            .saturating_add(self.commands_duration)
            .saturating_add(self.advance_duration)
    }

    pub fn overhead_duration(&self) -> Duration {
        self.wall_duration.saturating_sub(self.accounted_duration())
    }
}
