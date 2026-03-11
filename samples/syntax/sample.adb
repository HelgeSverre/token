-- Ada Syntax Highlighting Test
-- A concurrent task scheduler with strong typing and contracts.

with Ada.Text_IO;           use Ada.Text_IO;
with Ada.Integer_Text_IO;   use Ada.Integer_Text_IO;
with Ada.Float_Text_IO;     use Ada.Float_Text_IO;
with Ada.Calendar;           use Ada.Calendar;
with Ada.Containers.Vectors;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Exceptions;

procedure Task_Scheduler is

   -- ============================================================
   -- Type definitions with constraints
   -- ============================================================

   type Priority is (Low, Medium, High, Critical);
   for Priority use (Low => 0, Medium => 1, High => 2, Critical => 3);

   type Status is (Open, In_Progress, Done, Cancelled);

   subtype Task_Id is Positive range 1 .. 10_000;
   subtype Percentage is Float range 0.0 .. 100.0;
   subtype Worker_Count is Positive range 1 .. 64;

   type Tag_Array is array (Positive range <>) of Unbounded_String;
   type Tag_List is access Tag_Array;

   -- Task record with default values
   type Task_Record is record
      Id          : Task_Id;
      Title       : Unbounded_String;
      Description : Unbounded_String := Null_Unbounded_String;
      Status      : Task_Scheduler.Status := Open;
      Priority    : Task_Scheduler.Priority := Medium;
      Tags        : Tag_List := null;
      Created_At  : Time := Clock;
      Updated_At  : Time := Clock;
   end record;

   -- ============================================================
   -- Generic sorted container
   -- ============================================================

   generic
      type Element_Type is private;
      with function "<" (Left, Right : Element_Type) return Boolean is <>;
      with function "=" (Left, Right : Element_Type) return Boolean is <>;
   package Sorted_Lists is
      type List is tagged private;

      procedure Insert (Self : in out List; Item : Element_Type);
      function  Contains (Self : List; Item : Element_Type) return Boolean;
      function  Length (Self : List) return Natural;
      function  Get (Self : List; Index : Positive) return Element_Type
        with Pre => Index <= Self.Length;

   private
      package Vectors is new Ada.Containers.Vectors
        (Index_Type => Positive, Element_Type => Element_Type);

      type List is tagged record
         Items : Vectors.Vector;
      end record;
   end Sorted_Lists;

   package body Sorted_Lists is
      procedure Insert (Self : in out List; Item : Element_Type) is
         use Vectors;
         Inserted : Boolean := False;
      begin
         for I in Self.Items.First_Index .. Self.Items.Last_Index loop
            if Item < Self.Items (I) then
               Self.Items.Insert (Before => I, New_Item => Item);
               Inserted := True;
               exit;
            end if;
         end loop;

         if not Inserted then
            Self.Items.Append (Item);
         end if;
      end Insert;

      function Contains (Self : List; Item : Element_Type) return Boolean is
      begin
         for E of Self.Items loop
            if E = Item then
               return True;
            end if;
         end loop;
         return False;
      end Contains;

      function Length (Self : List) return Natural is
      begin
         return Natural (Self.Items.Length);
      end Length;

      function Get (Self : List; Index : Positive) return Element_Type is
      begin
         return Self.Items (Index);
      end Get;
   end Sorted_Lists;

   -- ============================================================
   -- Task store with protected type (thread-safe)
   -- ============================================================

   package Task_Vectors is new Ada.Containers.Vectors
     (Index_Type => Positive, Element_Type => Task_Record);

   protected type Task_Store is
      procedure Create
        (Title    : String;
         Prio     : Priority;
         Id       : out Task_Id);

      procedure Update_Status
        (Id         : Task_Id;
         New_Status : Status;
         Success    : out Boolean);

      procedure Delete
        (Id      : Task_Id;
         Success : out Boolean);

      function Get (Id : Task_Id) return Task_Record;
      function Count return Natural;
      function All_Tasks return Task_Vectors.Vector;

      function Count_By_Status (S : Status) return Natural;
      function Completion_Rate return Percentage;

   private
      Tasks   : Task_Vectors.Vector;
      Next_Id : Task_Id := 1;
   end Task_Store;

   protected body Task_Store is

      procedure Create
        (Title    : String;
         Prio     : Priority;
         Id       : out Task_Id)
      is
         New_Task : Task_Record;
      begin
         New_Task.Id       := Next_Id;
         New_Task.Title    := To_Unbounded_String (Title);
         New_Task.Priority := Prio;
         Tasks.Append (New_Task);
         Id := Next_Id;
         Next_Id := Next_Id + 1;
      end Create;

      procedure Update_Status
        (Id         : Task_Id;
         New_Status : Status;
         Success    : out Boolean)
      is
      begin
         Success := False;
         for I in Tasks.First_Index .. Tasks.Last_Index loop
            if Tasks (I).Id = Id then
               declare
                  T : Task_Record := Tasks (I);
               begin
                  T.Status     := New_Status;
                  T.Updated_At := Clock;
                  Tasks.Replace_Element (I, T);
                  Success := True;
               end;
               exit;
            end if;
         end loop;
      end Update_Status;

      procedure Delete
        (Id      : Task_Id;
         Success : out Boolean)
      is
      begin
         Success := False;
         for I in Tasks.First_Index .. Tasks.Last_Index loop
            if Tasks (I).Id = Id then
               Tasks.Delete (I);
               Success := True;
               exit;
            end if;
         end loop;
      end Delete;

      function Get (Id : Task_Id) return Task_Record is
      begin
         for T of Tasks loop
            if T.Id = Id then
               return T;
            end if;
         end loop;
         raise Constraint_Error with "Task not found:" & Task_Id'Image (Id);
      end Get;

      function Count return Natural is
      begin
         return Natural (Tasks.Length);
      end Count;

      function All_Tasks return Task_Vectors.Vector is
      begin
         return Tasks;
      end All_Tasks;

      function Count_By_Status (S : Status) return Natural is
         N : Natural := 0;
      begin
         for T of Tasks loop
            if T.Status = S then
               N := N + 1;
            end if;
         end loop;
         return N;
      end Count_By_Status;

      function Completion_Rate return Percentage is
         Total : constant Natural := Natural (Tasks.Length);
         Completed : Natural;
      begin
         if Total = 0 then
            return 0.0;
         end if;
         Completed := Count_By_Status (Done);
         return Percentage (Float (Completed) / Float (Total) * 100.0);
      end Completion_Rate;

   end Task_Store;

   -- ============================================================
   -- Worker task (Ada concurrency)
   -- ============================================================

   task type Worker (Id : Positive) is
      entry Process (T : Task_Record);
      entry Shutdown;
   end Worker;

   task body Worker is
      Current_Task : Task_Record;
      Running      : Boolean := True;
   begin
      while Running loop
         select
            accept Process (T : Task_Record) do
               Current_Task := T;
            end Process;

            Put_Line ("  Worker" & Positive'Image (Id) &
                      " processing: " &
                      To_String (Current_Task.Title));
            delay 0.1;  -- Simulate work

         or
            accept Shutdown do
               Running := False;
            end Shutdown;

         or
            terminate;
         end select;
      end loop;
   end Worker;

   -- ============================================================
   -- Display helpers
   -- ============================================================

   procedure Put_Status_Icon (S : Status) is
   begin
      case S is
         when Open        => Put ("[ ] ");
         when In_Progress => Put ("[~] ");
         when Done        => Put ("[x] ");
         when Cancelled   => Put ("[-] ");
      end case;
   end Put_Status_Icon;

   procedure Put_Priority_Icon (P : Priority) is
   begin
      case P is
         when Low      => Put ("  ");
         when Medium   => Put ("! ");
         when High     => Put ("!!");
         when Critical => Put ("!!!");
      end case;
   end Put_Priority_Icon;

   procedure Print_Task (T : Task_Record) is
   begin
      Put ("#");
      Put (T.Id, Width => 3);
      Put (" ");
      Put_Status_Icon (T.Status);
      Put_Priority_Icon (T.Priority);
      Put (" ");
      Put_Line (To_String (T.Title));
   end Print_Task;

   procedure Print_Stats (Store : in out Task_Store) is
      Rate : Percentage;
   begin
      New_Line;
      Put_Line ("=== Statistics ===");
      Put ("Total:       "); Put (Store.Count, Width => 1); New_Line;
      Put ("Open:        "); Put (Store.Count_By_Status (Open), Width => 1); New_Line;
      Put ("In Progress: "); Put (Store.Count_By_Status (In_Progress), Width => 1); New_Line;
      Put ("Done:        "); Put (Store.Count_By_Status (Done), Width => 1); New_Line;
      Put ("Cancelled:   "); Put (Store.Count_By_Status (Cancelled), Width => 1); New_Line;
      Rate := Store.Completion_Rate;
      Put ("Completion:  "); Put (Rate, Fore => 1, Aft => 1, Exp => 0); Put_Line ("%");
   end Print_Stats;

   -- ============================================================
   -- Main
   -- ============================================================

   Store   : Task_Store;
   Id      : Task_Id;
   Success : Boolean;

begin
   Put_Line ("Task Scheduler v1.0");
   New_Line;

   -- Create tasks
   Store.Create ("Implement syntax highlighting", High, Id);
   Store.Create ("Fix cursor blinking", Low, Id);
   Store.Create ("Add split view", Medium, Id);
   Store.Create ("Write documentation", Medium, Id);
   Store.Create ("Performance profiling", High, Id);

   -- Update statuses
   Store.Update_Status (1, In_Progress, Success);
   Store.Update_Status (2, Done, Success);

   -- Print all tasks
   Put_Line ("All tasks:");
   declare
      All : constant Task_Vectors.Vector := Store.All_Tasks;
   begin
      for T of All loop
         Print_Task (T);
      end loop;
   end;

   Print_Stats (Store);

exception
   when E : others =>
      Put_Line ("Error: " & Ada.Exceptions.Exception_Message (E));
end Task_Scheduler;
