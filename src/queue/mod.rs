use std::cmp::Ordering;
use std::collections::BinaryHeap;
use tokio::sync::Mutex;
use std::sync::Arc;
use tokio::time::{Duration, Instant};

/// A message that can be added to the queue.
///
/// Each message has an ID, content, a priority, and an availability time.
/// The priority determines the order in which messages are processed,
/// with higher values being processed first. The availability time specifies when
/// the message becomes available for processing.
#[derive(Debug, Clone, Eq)]
pub struct Message {
    /// The unique identifier of the message.
    pub id: u64,
    /// The content of the message.
    pub content: String,
    /// The priority of the message. Higher values indicate higher priority.
    pub priority: u8,
    /// The time when the message will be available for processing.
    pub available_at: Instant,
}

// Implement ordering for the message to be used in a priority queue.
impl Ord for Message {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare based on availability time first, then priority if available times are equal
        match self.available_at.cmp(&other.available_at) {
            Ordering::Equal => other.priority.cmp(&self.priority), // Higher priority messages come first
            other_order => other_order.reverse(), // Earlier availability comes first
        }
    }
}


impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Custom errors that can occur when interacting with the queue.
#[derive(Debug)]
pub enum QueueError {
    /// Error occurring when a lock cannot be acquired.
    LockError,
}

/// A thread-safe priority queue for managing `Message` objects with support for delayed processing and batch operations.
///
/// The `Queue` allows multiple producers and consumers to safely push and pop messages concurrently,
/// with messages being processed in order of their priority and availability time.
#[derive(Debug, Clone)]
pub struct Queue {
    messages: Arc<Mutex<BinaryHeap<Message>>>,
}

impl Queue {
    /// Creates a new, empty `Queue`.
    ///
    /// # Examples
    ///
    ///
    /// use hexboltmq::queue::Queue;
    /// let queue = Queue::new();
    ///
    pub fn new() -> Self {
        Queue {
            messages: Arc::new(Mutex::new(BinaryHeap::new())),
        }
    }

    /// Adds a message to the queue with an optional delay.
    ///
    /// Messages are stored based on their priority and availability time.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to add to the queue.
    /// * `delay` - The delay duration after which the message becomes available for processing.
    ///
    /// # Errors
    ///
    /// Returns `QueueError::LockError` if the queue lock cannot be acquired.
    ///
    /// # Examples
    ///
    ///
    /// use hexboltmq::queue::{Queue, Message};
    /// use tokio::time::Duration;
    /// let queue = Queue::new();
    /// queue.push(Message { id: 1, content: String::from("Hello"), priority: 5 }, Duration::from_secs(2)).await.unwrap();
    ///
    pub async fn push(&self, message: Message, delay: Duration) -> Result<(), QueueError> {
        // Calculate the availability time based on the current time and delay
        let available_at = Instant::now() + delay;

        // Create a new message with the updated availability time
        let delayed_message = Message { available_at, ..message };

        // Lock the queue and push the message
        let mut queue = self.messages.lock().await;
        queue.push(delayed_message.clone());
        println!("Message pushed: {:?}", delayed_message);

        Ok(())
    }

    /// Removes and returns the highest priority message from the queue that is available for processing.
    ///
    /// Messages that are not yet available due to a delay are not returned.
    ///
    /// Returns `None` if the queue is empty or if no messages are currently available.
    ///
    /// # Errors
    ///
    /// Returns `QueueError::LockError` if the queue lock cannot be acquired.
    ///
    /// # Examples
    ///
    ///
    /// use hexboltmq::queue::{Queue, Message};
    /// use tokio::time::Duration;
    /// let queue = Queue::new();
    /// queue.push(Message { id: 1, content: String::from("Hello"), priority: 5 }, Duration::from_secs(0)).await.unwrap();
    /// let msg = queue.pop().await.unwrap();
    /// assert_eq!(msg.unwrap().priority, 5);
    ///
    pub async fn pop(&self) -> Result<Option<Message>, QueueError> {
        let mut queue = self.messages.lock().await;

        // Check if the top message is available for processing
        if let Some(top_message) = queue.peek() {
            if top_message.available_at <= Instant::now() {
                let msg = queue.pop();
                if let Some(ref m) = msg {
                    println!("Message popped: {:?}", m);
                }
                return Ok(msg);
            }
        }

        // Return None if no messages are available for processing
        Ok(None)
    }

    /// Removes and returns up to `batch_size` highest priority messages from the queue that are available for processing.
    ///
    /// Messages that are not yet available due to a delay are not returned.
    ///
    /// Returns an empty vector if no messages are currently available.
    ///
    /// # Arguments
    ///
    /// * `batch_size` - The maximum number of messages to retrieve in one batch.
    ///
    /// # Errors
    ///
    /// Returns `QueueError::LockError` if the queue lock cannot be acquired.
    ///
    /// # Examples
    ///
    ///
    /// use hexboltmq::queue::{Queue, Message};
    /// use tokio::time::Duration;
    /// let queue = Queue::new();
    /// queue.push(Message { id: 1, content: String::from("Hello"), priority: 5 }, Duration::from_secs(0)).await.unwrap();
    /// queue.push(Message { id: 2, content: String::from("World"), priority: 10 }, Duration::from_secs(0)).await.unwrap();
    /// let messages = queue.pop_batch(2).await.unwrap();
    /// assert_eq!(messages.len(), 2);
    ///
    pub async fn pop_batch(&self, batch_size: usize) -> Result<Vec<Message>, QueueError> {
        let mut queue = self.messages.lock().await;
        let mut batch = Vec::new();
    
        while batch.len() < batch_size {
            if let Some(top_message) = queue.peek() {
                println!(
                    "Checking top message: id={}, priority={}, available_at={:?}, now={:?}",
                    top_message.id, top_message.priority, top_message.available_at, Instant::now()
                );
    
                if top_message.available_at <= Instant::now() {
                    if let Some(msg) = queue.pop() {
                        println!("Popped message: {:?}", msg);
                        batch.push(msg);
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    
        println!("Batch size after pop: {}", batch.len());
        Ok(batch)
    }
    
    

    /// Returns the current size of the queue.
    ///
    /// # Errors
    ///
    /// Returns `QueueError::LockError` if the queue lock cannot be acquired.
    ///
    /// # Examples
    ///
    ///
    /// use hexboltmq::queue::{Queue, Message};
    /// let queue = Queue::new();
    /// assert_eq!(queue.size().await.unwrap(), 0);
    /// queue.push(Message { id: 1, content: String::from("Hello"), priority: 5 }, Duration::from_secs(0)).await.unwrap();
    /// assert_eq!(queue.size().await.unwrap(), 1);
    ///
    pub async fn size(&self) -> Result<usize, QueueError> {
        let queue = self.messages.lock().await;
        Ok(queue.len())
    }
}
