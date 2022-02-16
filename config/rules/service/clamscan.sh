#!/bin/bash

set -e

echo $1 | clamscan -

exit $?
