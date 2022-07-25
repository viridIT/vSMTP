/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
 */
use crate::Args;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use vsmtp_config::Config;

/// Initialize the tracing subsystem.
pub fn initialize(args: &Args, config: &Config) {
    let writer_backend = tracing_appender::rolling::daily(&config.server.logs.filepath, "vsmtp")
        .with_filter(|metadata| !metadata.target().starts_with("app"));

    let writer_app = tracing_appender::rolling::daily(&config.app.logs.filepath, "app")
        .with_filter(|metadata| metadata.target().starts_with("app"));

    #[cfg(debug_assertions)]
    let layer = fmt::layer()
        .pretty()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true);

    #[cfg(not(debug_assertions))]
    let layer = fmt::layer()
        .compact()
        .with_thread_ids(true)
        .with_target(true);

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::builder().try_from_env().unwrap_or_else(|_| {
            let mut e = EnvFilter::default();
            for i in &config.server.logs.level {
                e = e.add_directive(i.clone());
            }
            e
        }))
        .with(layer);

    if args.no_daemon {
        subscriber
            .with(fmt::layer().with_writer(writer_backend.and(writer_app).and(std::io::stdout)))
            .init();
    } else {
        subscriber
            .with(
                fmt::layer()
                    .with_writer(writer_backend.and(writer_app))
                    .with_ansi(false),
            )
            .init();
    }
}
