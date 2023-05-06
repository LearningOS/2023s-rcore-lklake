//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
/*########################################################## */
use alloc::collections::LinkedList;
/*########################################################## */
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: LinkedList<Arc<TaskControlBlock>>,
}

/*########################################################## */
#[allow(unused)]
static BIGSTRIDE:u64 = 0x7fffffffffffffff;
/*########################################################## */
/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
/*########################################################## */
            ready_queue: LinkedList::new(),
/*########################################################## */
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
/*########################################################## */
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let mut next_proc:Option<&Arc<TaskControlBlock>> = None;
        let mut i:isize = -1;
        for (index,p) in self.ready_queue.iter().enumerate(){
            let inner = p.inner_exclusive_access();
            if inner.task_status == crate::task::TaskStatus::Ready{
                if next_proc.is_none() {
                    next_proc = Some(p);
                    i = index as isize;
                    continue;
                }
                let p_stride = inner.stride;
                let next_stride =  next_proc.unwrap().inner_exclusive_access().stride;
                if ((p_stride - next_stride)as i64) <0{
                    next_proc = Some(p);
                    i = index as isize;
                } 
            }
        }
        if i==-1{
            panic!("all apps are over!");
        }
        let result = self.ready_queue.remove(i as usize);
        let priority = result.inner_exclusive_access().priority;
        result.inner_exclusive_access().stride += BIGSTRIDE/priority as u64;
        Some(result)
    }
/*########################################################## */
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
