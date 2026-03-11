// SPDX-License-Identifier: MIT
// Solidity Syntax Highlighting Test
// A task bounty board with escrow, deadlines, and dispute resolution.

pragma solidity ^0.8.24;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

// ============================================================
// Interfaces and types
// ============================================================

interface IArbitrator {
    function resolveDispute(uint256 disputeId) external returns (address winner);
    function fee() external view returns (uint256);
}

// Custom errors (gas efficient)
error TaskNotFound(uint256 taskId);
error InvalidStatus(uint256 taskId, TaskStatus current, TaskStatus expected);
error InsufficientBounty(uint256 provided, uint256 minimum);
error DeadlinePassed(uint256 taskId, uint256 deadline);
error NotAssignee(uint256 taskId, address caller, address assignee);
error TransferFailed(address to, uint256 amount);

// ============================================================
// Enums and structs
// ============================================================

enum TaskStatus {
    Open,
    Assigned,
    InReview,
    Completed,
    Disputed,
    Cancelled
}

enum Priority {
    Low,
    Medium,
    High,
    Critical
}

struct Task {
    uint256 id;
    address creator;
    address assignee;
    string title;
    string description;
    TaskStatus status;
    Priority priority;
    string[] tags;
    uint256 bounty;
    uint256 deadline;
    uint256 createdAt;
    uint256 completedAt;
}

struct TaskStats {
    uint256 total;
    uint256 open;
    uint256 completed;
    uint256 totalBountyLocked;
    uint256 totalBountyPaid;
}

// ============================================================
// Events
// ============================================================

event TaskCreated(
    uint256 indexed taskId,
    address indexed creator,
    string title,
    uint256 bounty,
    Priority priority
);

event TaskAssigned(
    uint256 indexed taskId,
    address indexed assignee
);

event TaskSubmitted(
    uint256 indexed taskId,
    address indexed assignee
);

event TaskCompleted(
    uint256 indexed taskId,
    address indexed assignee,
    uint256 bountyPaid
);

event TaskCancelled(
    uint256 indexed taskId,
    address indexed creator,
    uint256 bountyRefunded
);

event DisputeRaised(
    uint256 indexed taskId,
    address indexed disputant
);

// ============================================================
// Main contract
// ============================================================

/// @title TaskBounty - Decentralized task bounty board
/// @author Token Editor Team
/// @notice Create, assign, and complete tasks with ETH bounties
/// @dev Uses escrow pattern for bounty management
contract TaskBounty is Ownable, ReentrancyGuard {

    // ---- State variables ----

    uint256 private _nextTaskId = 1;
    uint256 public platformFeePercent = 2; // 2%
    uint256 public constant MIN_BOUNTY = 0.001 ether;
    uint256 public constant MAX_DEADLINE = 365 days;

    mapping(uint256 => Task) private _tasks;
    mapping(address => uint256[]) private _creatorTasks;
    mapping(address => uint256[]) private _assigneeTasks;
    mapping(string => uint256[]) private _tagIndex;

    uint256 public totalTasks;
    uint256 public totalBountyLocked;
    uint256 public totalBountyPaid;
    uint256 public platformBalance;

    IArbitrator public arbitrator;

    // ---- Modifiers ----

    modifier taskExists(uint256 taskId) {
        if (_tasks[taskId].id == 0) revert TaskNotFound(taskId);
        _;
    }

    modifier inStatus(uint256 taskId, TaskStatus expected) {
        TaskStatus current = _tasks[taskId].status;
        if (current != expected) revert InvalidStatus(taskId, current, expected);
        _;
    }

    modifier onlyCreator(uint256 taskId) {
        require(msg.sender == _tasks[taskId].creator, "Not task creator");
        _;
    }

    modifier onlyAssignee(uint256 taskId) {
        address assignee = _tasks[taskId].assignee;
        if (msg.sender != assignee) revert NotAssignee(taskId, msg.sender, assignee);
        _;
    }

    modifier beforeDeadline(uint256 taskId) {
        uint256 deadline = _tasks[taskId].deadline;
        if (block.timestamp > deadline) revert DeadlinePassed(taskId, deadline);
        _;
    }

    // ---- Constructor ----

    constructor(address _arbitrator) Ownable(msg.sender) {
        if (_arbitrator != address(0)) {
            arbitrator = IArbitrator(_arbitrator);
        }
    }

    // ---- Core functions ----

    /// @notice Create a new task with a bounty
    /// @param title Task title
    /// @param description Task description
    /// @param priority Task priority level
    /// @param tags Array of tag strings
    /// @param deadline Unix timestamp for task deadline
    /// @return taskId The ID of the created task
    function createTask(
        string calldata title,
        string calldata description,
        Priority priority,
        string[] calldata tags,
        uint256 deadline
    ) external payable returns (uint256 taskId) {
        if (msg.value < MIN_BOUNTY) {
            revert InsufficientBounty(msg.value, MIN_BOUNTY);
        }
        require(deadline > block.timestamp, "Deadline must be in the future");
        require(deadline <= block.timestamp + MAX_DEADLINE, "Deadline too far");
        require(bytes(title).length > 0, "Title required");

        taskId = _nextTaskId++;

        Task storage task = _tasks[taskId];
        task.id = taskId;
        task.creator = msg.sender;
        task.title = title;
        task.description = description;
        task.status = TaskStatus.Open;
        task.priority = priority;
        task.tags = tags;
        task.bounty = msg.value;
        task.deadline = deadline;
        task.createdAt = block.timestamp;

        _creatorTasks[msg.sender].push(taskId);

        // Index by tags
        for (uint256 i = 0; i < tags.length; i++) {
            _tagIndex[tags[i]].push(taskId);
        }

        totalTasks++;
        totalBountyLocked += msg.value;

        emit TaskCreated(taskId, msg.sender, title, msg.value, priority);
    }

    /// @notice Claim an open task
    function claimTask(uint256 taskId)
        external
        taskExists(taskId)
        inStatus(taskId, TaskStatus.Open)
        beforeDeadline(taskId)
    {
        require(msg.sender != _tasks[taskId].creator, "Creator cannot claim own task");

        _tasks[taskId].assignee = msg.sender;
        _tasks[taskId].status = TaskStatus.Assigned;

        _assigneeTasks[msg.sender].push(taskId);

        emit TaskAssigned(taskId, msg.sender);
    }

    /// @notice Submit completed work for review
    function submitWork(uint256 taskId)
        external
        taskExists(taskId)
        inStatus(taskId, TaskStatus.Assigned)
        onlyAssignee(taskId)
        beforeDeadline(taskId)
    {
        _tasks[taskId].status = TaskStatus.InReview;
        emit TaskSubmitted(taskId, msg.sender);
    }

    /// @notice Approve submitted work and release bounty
    function approveWork(uint256 taskId)
        external
        nonReentrant
        taskExists(taskId)
        inStatus(taskId, TaskStatus.InReview)
        onlyCreator(taskId)
    {
        Task storage task = _tasks[taskId];
        task.status = TaskStatus.Completed;
        task.completedAt = block.timestamp;

        // Calculate fees
        uint256 fee = (task.bounty * platformFeePercent) / 100;
        uint256 payout = task.bounty - fee;

        platformBalance += fee;
        totalBountyLocked -= task.bounty;
        totalBountyPaid += payout;

        // Transfer bounty to assignee
        (bool success, ) = payable(task.assignee).call{value: payout}("");
        if (!success) revert TransferFailed(task.assignee, payout);

        emit TaskCompleted(taskId, task.assignee, payout);
    }

    /// @notice Cancel an open task and refund bounty
    function cancelTask(uint256 taskId)
        external
        nonReentrant
        taskExists(taskId)
        onlyCreator(taskId)
    {
        Task storage task = _tasks[taskId];
        require(
            task.status == TaskStatus.Open || task.status == TaskStatus.Assigned,
            "Cannot cancel in current status"
        );

        uint256 refund = task.bounty;
        task.status = TaskStatus.Cancelled;
        totalBountyLocked -= refund;

        (bool success, ) = payable(task.creator).call{value: refund}("");
        if (!success) revert TransferFailed(task.creator, refund);

        emit TaskCancelled(taskId, task.creator, refund);
    }

    // ---- View functions ----

    /// @notice Get task details
    function getTask(uint256 taskId) external view taskExists(taskId) returns (Task memory) {
        return _tasks[taskId];
    }

    /// @notice Get tasks created by an address
    function getCreatorTasks(address creator) external view returns (uint256[] memory) {
        return _creatorTasks[creator];
    }

    /// @notice Get overall statistics
    function getStats() external view returns (TaskStats memory) {
        uint256 openCount = 0;
        uint256 completedCount = 0;

        for (uint256 i = 1; i < _nextTaskId; i++) {
            if (_tasks[i].status == TaskStatus.Open) openCount++;
            if (_tasks[i].status == TaskStatus.Completed) completedCount++;
        }

        return TaskStats({
            total: totalTasks,
            open: openCount,
            completed: completedCount,
            totalBountyLocked: totalBountyLocked,
            totalBountyPaid: totalBountyPaid
        });
    }

    // ---- Admin functions ----

    function setFeePercent(uint256 newFee) external onlyOwner {
        require(newFee <= 10, "Fee too high");
        platformFeePercent = newFee;
    }

    function withdrawFees() external onlyOwner nonReentrant {
        uint256 amount = platformBalance;
        platformBalance = 0;

        (bool success, ) = payable(owner()).call{value: amount}("");
        if (!success) revert TransferFailed(owner(), amount);
    }

    /// @notice Accept ETH deposits
    receive() external payable {}
}
