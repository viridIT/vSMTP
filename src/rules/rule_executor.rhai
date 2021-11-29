let status = __CONTINUE();

for r in __rules {
    if r.when == __step {
        let status = switch r.condition() {
            true => r.on_success(),
            false => r.on_failure(),
        };

        if status != __CONTINUE() {
            return status;
        }
    }
}

// in this case, status == CONTINUE, default is used.
status