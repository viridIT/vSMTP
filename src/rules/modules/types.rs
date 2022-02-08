/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use rhai::plugin::*;
#[allow(dead_code)]
#[export_module]
pub mod types {

    #[rhai_fn(get = "stdout", return_raw)]
    pub fn stdout(this: &mut std::process::Output) -> Result<String, Box<EvalAltResult>> {
        Ok(std::str::from_utf8(&this.stdout)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .to_string())
    }

    #[rhai_fn(get = "stderr", return_raw)]
    pub fn stderr(this: &mut std::process::Output) -> Result<String, Box<EvalAltResult>> {
        Ok(std::str::from_utf8(&this.stderr)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .to_string())
    }

    #[rhai_fn(get = "code", return_raw)]
    pub fn code(this: &mut std::process::Output) -> Result<i64, Box<EvalAltResult>> {
        Ok(this.status.code().ok_or_else::<Box<EvalAltResult>, _>(|| {
            "a SHELL process have been terminated by a signal".into()
        })? as i64)
    }
}
