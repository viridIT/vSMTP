/*
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
*/

// standard library for SMTP codes

// Relay
object code554_7_1 code = #{ code: 554, enhanced: "5.7.1", text: "Relay access denied" };

// Email authentication status codes
//
// See. RFC 7372 for updated codes RFC
//

// DKIM Failure codes (RFC 6376)
object code550_7_20 code = #{ code: 550, enhanced: "5.7.20", text: "No passing DKIM signature found" };
object code550_7_21 code = #{ code: 550, enhanced: "5.7.21", text: "No acceptable DKIM signature found" };
object code550_7_22 code = #{ code: 550, enhanced: "5.7.22", text: "No valid author-matched DKIM signature found" };

// SPF Failure codes (RFC 7208)
object code550_7_23 code = #{ code: 550, enhanced: "5.7.23", text: "SPF validation failed" };
object code451_7_24 code = #{ code: 451, enhanced: "5.7.24", text: "SPF validation error" };
object code550_7_24 code = #{ code: 550, enhanced: "5.7.24", text: "SPF validation error" };

// Reverse DNS Failure code
object code550_7_25 code = #{ code: 550, enhanced: "5.7.25", text: "Reverse DNS validation failed" };

// Multiple Authentication Failures code
object code550_7_26 code = #{ code: 500, enhanced: "5.7.26", text: "Multiple authentication checks failed" };

// Null MX codes (RFC 7505)
object code550_7_27 code = #{ code: 550, enhanced: "5.7.27", text: "Sender address has null MX" };
object code556_1_10 code = #{ code: 556, enhanced: "5.1.10", text: "Recipient address has null MX" };
