//! Process management syscalls
use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,
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
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
/*####################################################### */
    trace!("kernel: sys_get_time");
    let us = crate::timer::get_time_us();
    let _time_val = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
    };

    crate::mm::copy_to_user(crate::task::current_user_token(),&_time_val as *const TimeVal as *const u8 ,_ts as *const u8,core::mem::size_of::<TimeVal>());
    0
/*####################################################### */
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
/*####################################################### */
    trace!("kernel: sys_task_info");
    
    let _task_info = TaskInfo{
            status : TaskStatus::Running,
            syscall_times : crate::task::get_current_intr_record(),
            time : crate::timer::get_time_ms() - crate::task::get_current_start_time(),
    };
    crate::mm::copy_to_user(crate::task::current_user_token(),&_task_info as *const TaskInfo as *const u8 ,_ti as *const u8,core::mem::size_of::<TaskInfo>());
    0
/*####################################################### */
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
/*####################################################### */
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
/*####################################################### */
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
/*####################################################### */
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
/*####################################################### */
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
