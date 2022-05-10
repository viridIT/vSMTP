#!/bin/bash

set -e

systemctl stop vsmtp
systemctl disable vsmtp
systemctl enable postfix

/home/lala/.cargo/bin/hyperfine \
    -L mail 1,10 \
    -L session 1,10 \
    --prepare 'systemctl restart postfix; sleep 1' \
    'smtp-source -s {session} -l 5120 -m {mail} -c -f to@local.com -N -t postfix@example.com localhost:25' \
    --export-json diff-postfix.json \
    --show-output


systemctl stop postfix
systemctl disable postfix
systemctl enable vsmtp

/home/lala/.cargo/bin/hyperfine \
    -L mail 1,10 \
    -L session 1,10 \
    --prepare 'systemctl restart vsmtp; sleep 1' \
    'smtp-source -s {session} -l 5120 -m {mail} -c -f to@local.com -N -t vsmtp@example.com localhost:25' \
    --export-json diff-vsmtp.json \
    --show-output

jq -s '.[0].results=([.[].results]|flatten)|.[0]' diff-postfix.json diff-vsmtp.json > diff.json

## You can then visualize the data with `https://github.com/sharkdp/hyperfine/tree/master/scripts`

# `python3 plot_whisker.py diff.json`
