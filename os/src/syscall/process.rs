//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    config::MAX_SYSCALL_NUM,
    loader::get_app_data_by_name,
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
/*####################################################### */

pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = crate::timer::get_time_us();
    let _time_val = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
    };

    crate::mm::copy_to_user(crate::task::current_user_token(),&_time_val as *const TimeVal as *const u8 ,_ts as *const u8,core::mem::size_of::<TimeVal>());
    0
}
/*####################################################### */

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
/*####################################################### */

pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
        
    let _task_info = TaskInfo{
            status : TaskStatus::Running,
            syscall_times : crate::task::get_current_intr_record(),
            time : crate::timer::get_time_ms() - crate::task::get_current_start_time(),
    };
    crate::mm::copy_to_user(crate::task::current_user_token(),&_task_info as *const TaskInfo as *const u8 ,_ti as *const u8,core::mem::size_of::<TaskInfo>());
    0
}
/*####################################################### */

/// YOUR JOB: Implement mmap.
/*####################################################### */

pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

    // fall early

    if(_port & (!0x7)) !=0{
        error!("mmap:port contain dirty bits:{}",_port);
        return -1;
    }
    if(_port & 0x7) == 0 {
        error!("mmap:request non R or W or X memory is meaningless:{}",_port);
        return -1;
    }
    if _start % crate::config::PAGE_SIZE != 0 {
        error!("mmap:start address is not page aligned:{}",_port);
        return -1;
    }
    if _len == 0{
        return 0;
    }
    if _len > 1024 * 1024 * 1024 {
        error!("mmap:too large");
        return -1;
    }

    // do the job

    let mut perm: crate::mm::PTEFlags = crate::mm::PTEFlags::U;
    info!("mmap:{}",_port);
    if (_port & 0b1) != 0 {
        perm |= crate::mm::PTEFlags::R;
        info!("mmap:R");
    }
    if (_port & 0b10 ) != 0{
        perm |= crate::mm::PTEFlags::W;
        info!("mmap:W");
    }
    if (_port & 0b100 ) != 0{
        perm |= crate::mm::PTEFlags::X;
        info!("mmap:X");
    }
    info!("perm:{}",perm.bits());
    let pagetable = crate::mm::PageTable::from_token(crate::task::current_user_token());

    let mut start = _start;
    let end = _start + _len;
    while start < end {
        let pte = pagetable.translate(crate::mm::VirtAddr( start).floor());
        if pte.is_some() && pte.unwrap().is_valid() {
            error!("mmap:already exist {:#x}",start);
            return -1;
        }
        start += crate::config::PAGE_SIZE;
    }
    start = _start;
    while start < end {
        println!("mapping:{:#x} {}",start,crate::mm::VirtAddr( start).floor().0);
        crate::task::task_mmap(
            crate::mm::VirtAddr( start).floor(), 
            perm
        );
        
        let pte = pagetable.translate(crate::mm::VirtAddr( start).floor());
        if pte.is_none() {
            error!("mmap:pte is null {:#x}",start);
            return -1;
        }
        if !pte.unwrap().is_valid(){
            error!("mmap:pte not valid {:#x}",start);
            return -1;
        }

        start += crate::config::PAGE_SIZE;
    }
    return 0;
}
/*####################################################### */

/// YOUR JOB: Implement munmap.
/*####################################################### */
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");

    // fall early
    let start_va = crate::mm::VirtAddr(_start);
    if !start_va.aligned(){
        error!("munmap:start address is not page aligned");
        return -1;
    }

    if _len == 0{
        return 0;
    }
    if _len % crate::config::PAGE_SIZE != 0{
        // not specified, choose to up align len
    }
    // let pagetable = &mut crate::task::get_current_task().memory_set.page_table;

    let pagetable = crate::mm::PageTable::from_token(crate::task::current_user_token());
    let mut start = _start;
    let end = _start +_len;

    while start < end {
        println!("unmapping:{:#x} {}",start,crate::mm::VirtAddr( start).floor().0);
        let pte = pagetable.translate(crate::mm::VirtAddr( start).floor());
        if pte.is_none() {
            error!("munmap:pte is null {:#x}",start);
            return -1;
        }
        if !pte.unwrap().is_valid(){
            error!("munmap:pte not valid {:#x}",start);
            return -1;
        }
        start += crate::config::PAGE_SIZE;
    }
    start = _start;

    while start < end {
        crate::task::task_unmap(
            crate::mm::VirtAddr( start).floor(), 
        );
        start += crate::config::PAGE_SIZE;
    }
    return 0;
}
/*####################################################### */

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/*########################################################## */
/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, _path);

    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let new_task = current_task().unwrap().spawn(data);
        let new_pid = new_task.pid.0;
        // modify trap context of new_task, because it returns immediately after switching
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        // we do not have to move to next instruction since we have done it before
        // for child process, fork returns 0
        trap_cx.x[10] = 0;
        // add new task to scheduler
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}
/*########################################################## */
// YOUR JOB: Set task priority.
/*########################################################## */
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio < 2{
        return -1;
    }
    current_task().unwrap().inner_exclusive_access().priority = _prio as u64;
    _prio
}
/*########################################################## */