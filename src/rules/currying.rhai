/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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

let vsl = #{

    // checks if the object matches the connection data.
    IS_CONNECT: |object| {
        __is_connect(connect, object)
    },

    // checks if the object matches the HELO command content.
    IS_HELO: |object| {
        __is_helo(helo, object)
    },

    // checks if the object matches the MAIL FROM command content.
    IS_MAIL: |object| {
        __is_mail(mail, object)
    },

    // checks if the object matches one of the recipients given by all RCPT TO commands.
    IS_RCPT: |object| {
        __is_rcpt(rcpt, object)
    },

    // is useless for the time being.
    IS_DATA: |object| {
        __is_data(data, object)
    },

    // returns a state that will influence vsmtp's state machine, if returned by
    // a on_success or on_failure rule callback.
    ACCEPT:   || __ACCEPT(),
    FACCEPT:  || __FACCEPT(),
    CONTINUE: || __CONTINUE(),
    DENY:     || __DENY(),

    // logs a message to stdout.
    LOG: |message| {
        __LOG(message, "stdout");
    },

    // logs a message to stderr.
    LOG_ERR: |message| {
        __LOG(message, "stderr");
    },

    // logs a message to a file.
    LOG_FILE: |message, path| {
        __LOG(message, path);
    },

    // change the content of the HELO command.
    REWRITE_HELO: |value| {
        helo = value;
    },

    // change the content of the MAIL FROM command.
    REWRITE_MAIL_FROM: |value| {
        mail = value;
    },

    // change one of the recipient specified by the RCPT TO command.
    REWRITE_RCPT: |to_rewrite, value| {
        for (recipient, index) in rcpt {
            if recipient == to_rewrite {
                rcpt.custom_replace(index, value);
                break;
            }
        }
    },

    // add a recipient to the list.
    ADD_RCPT: |to_add| {
        rcpt.push(to_add);
    },

    // remove a recipient from the list.
    REMOVE_RCPT: |to_remove| {
        for (recipient, index) in rcpt {
            if recipient == to_remove {
                rcpt.custom_remove(index);
                break;
            }
        }
    },

    // write the current mail to a file.
    WRITE: |path| {
        __WRITE(data, path);
    },

    // write the current evenlop and mail content to a file in json format.
    DUMP: |path| {
        __DUMP(helo, mail, rcpt, data, path);
    },

    // write a mail to the quarantine folder.
    // the QUARANTINE action is defered in the operation queue and is executed
    // after the preq stage.
    QUARANTINE: |reason| {
        __OPERATION_QUEUE.op_quarantine(reason);
    },

    // send a mail.
    MAIL: |from, to, subject, body| {
        __MAIL(from, to, subject, body);
    },
};
