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

/// the OperationQueue is used to yield expensive operations
/// and executing them using rust's context instead of rhai's.
#[derive(Default, Debug, Clone)]
pub struct OperationQueue(Vec<Operation>);

/// an Operation can be pushed on top of the queue.
/// each operation triggers a specific action after
/// the preq stage.
#[derive(Debug, Clone)]
pub enum Operation {
    /// change the content of a header (header, value)
    MutateHeader(String, String),
    /// block an incoming email (blocked email directory)
    Block(String),
}

impl OperationQueue {
    /// push a new operation to process at the end of the queue.
    pub fn enqueue(&mut self, op: Operation) {
        self.0.push(op);
    }

    /// remove the first element in the queue.
    pub fn dequeue(&mut self) -> Option<Operation> {
        if self.0.is_empty() {
            None
        } else {
            Some(self.0.remove(0))
        }
    }
}

/// using into iterator we can abstract the queue
/// emptying system.
impl IntoIterator for OperationQueue {
    type Item = Operation;
    type IntoIter = QueueConsumeIterator;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter { 0: self }
    }
}

/// this a wrapper struct over the operation queue to
/// enable iteration.
pub struct QueueConsumeIterator(OperationQueue);

impl Iterator for QueueConsumeIterator {
    type Item = Operation;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.dequeue()
    }
}
