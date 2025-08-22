// Refrences:
//? https://pages.cs.wisc.edu/~markhill/restricted/arm_isa_quick_reference.pdf

/* 
 * this is so fucked
 * 
 * because of the entry point bs i put in, my variables and labels are all offset by one
 * problem is, idk if there will be an out-of-bounds edge case anywhere, need to fix this!
 * a line somewhere: self.instructions[line - 1]
 * 
 * Me but 2 months later, idk if this is still an issue and im lazy so ill find out one
 * day in the worst moment possible! :)
 */

/*
|------------|-------------------|----------------------------------|
| Input      | Type              | Result                           |
|------------|-------------------|----------------------------------|
| `MYVAR`    | Memory reference  | Resolved later via label table   |
| `0x2A`     | Hex               | Interpreted as `42`              |
| `42`       | Decimal           | Interpreted as `42`              |
| `0b101010` | Binary            | Interpreted as `42`              |
| `AX`       | Register          | Encoded as register              |
| `R0`       | Register          | Encoded as register AX           |
|------------|-------------------|----------------------------------|
*/

use std::collections::HashMap;
use std::env;
use std::fs;

type Instruction = u16;

const OPCODE_SHIFT: u16 = 10;
const MODE_SHIFT: u16 = 8;
const OPERANDS_WIDTH: u16 = 8;

// changing this past OPERANDS_WIDTH/2 will be catastrophic because of 2 operand mnemonics
const REGISTER_WIDTH: u16 = 4;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: assembler <input_file> [output_file]");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = if args.len() > 2 {
        &args[2]
    } else {
        "assembled.txt"
    };

    let lines = fs::read_to_string(input_path).expect("Failed to read file");
    let mut assembler = Assembler::new();
    assembler.assemble(&lines);
    let instructions = assembler.instructions;
    let output = instructions
        .iter()
        .enumerate()
        .map(|(i, instr)| {
            if i != instructions.len() - 1 {
                format!("{:04x}\n", instr)
            } else {
                format!("{:04x}", instr)
            }
        })
        .collect::<String>();

    fs::write(output_path, output).expect("Failed to write output file");
    println!("Assembly successful! Output written to {}", output_path);
}

struct Assembler {
    instructions: Vec<Instruction>,
    mnemonic_map: HashMap<&'static str, u16>,
    register_map: HashMap<&'static str, u16>,
    variable_map: HashMap<String, u16>,
    variable_memory_locations: HashMap<String, usize>,
    variable_references: HashMap<usize, String>,
    jump_references: HashMap<usize, String>,
    source_to_instruction_index: HashMap<usize, usize>,
    instruction_index: usize,
    entry_line: usize
}

impl Assembler {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            mnemonic_map: [
                ("NOP", 0x00), ("LODM", 0x01), ("LOD", 0x02), ("LODI", 0x03),
                ("STR", 0x05), ("STRI", 0x07),
                ("SWP", 0x08), ("MOV", 0x09), ("PUSH", 0x0A), ("POP", 0x0B),
                ("IN", 0x0C), ("OUT", 0x0D), ("HLT", 0x0F),
                ("JMP", 0x11), ("JZ", 0x12), ("JGZ", 0x13), ("JLZ", 0x14),
                ("JS", 0x15), ("JP", 0x16), ("JO", 0x17),
                ("CALL", 0x18), ("RET", 0x19),
                ("ADD", 0x20), ("SUB", 0x21), ("MUL", 0x24), ("IMUL", 0x25),
                ("DIV", 0x26), ("IDIV", 0x27), ("INC", 0x28), ("DEC", 0x29),
                ("CMP", 0x2A),
                ("AND", 0x30), ("NOT", 0x31), ("OR", 0x32), ("XOR", 0x33),
                ("SET", 0x34),
                ("LSL", 0x36), ("LSR", 0x37), ("ASL", 0x38), ("ASR", 0x39),
                ("ROL", 0x3A), ("ROR", 0x3B), ("RCL", 0x3C), ("RCR", 0x3D),
                ("ESC", 0x3F),
            ].iter().cloned().collect(),

            register_map: [
                ("AX", 0b000), ("BX", 0b001), ("CX", 0b010), ("DX", 0b011),
                ("SP", 0b100), ("PC", 0b101), ("MAR", 0b110), ("IR", 0b111),

                // Same registers, different names
                ("R0", 0b000), ("R1", 0b001), ("R2", 0b010), ("R3", 0b011),
                ("R4", 0b100), ("R5", 0b101), ("R6", 0b110), ("R7", 0b111),
            ].iter().cloned().collect(),

            variable_map: HashMap::new(),
            variable_memory_locations: HashMap::new(),
            variable_references: HashMap::new(),
            jump_references: HashMap::new(),
            source_to_instruction_index: HashMap::new(),
            instruction_index: 1,
            entry_line: 1usize,
        }
    }

    fn assemble(&mut self, content: &str) {
        let lines: Vec<&str> = content.lines().collect();
        self.process_file(&lines);
        self.resolve_variable_locations();
        self.resolve_jump_locations();

        if self.entry_line == 2 {
            self.entry_line = self.source_to_instruction_index
                .iter()
                .find(|&(_, &v)| v == 1)
                .map(|(&k, _)| k)
                .expect("Couldn't find entry line pointing to instruction 1");
        }

        if let Some(&entry_index) = self.source_to_instruction_index.get(&self.entry_line) {
            let jmp_opcode = self.mnemonic_map["JMP"] << OPCODE_SHIFT;
            let jmp_instr = jmp_opcode + (entry_index as u16);
            self.instructions.insert(0, jmp_instr as Instruction);
        } else {
            panic!("ENTRY point found but no instruction index mapped for it.");
        }
    }

    fn process_file(&mut self, lines: &[&str]) {
        let mut main_found = false;
        let mut variables_found = false;
        let mut source_line_num = 0;

        for line in lines.iter() {
            let line = line.to_uppercase();
            source_line_num += 1;

            if line.trim().starts_with(';') || line.trim().is_empty() {
                continue;
            }

            if line.trim() == "ENTRY:" {
                self.entry_line = source_line_num;
                continue;
            }

            if !main_found {
                if line.trim() == "CODE:" {
                    main_found = true;
                }
                continue;
            }

            if line.trim() == "DATA:" {
                variables_found = true;
                continue;
            }

            if variables_found {
                let tokens = Self::tokenize_variable(&line);
                let name = tokens[0].to_string();
                let value = tokens[1].parse::<u16>().unwrap_or(0);
                self.variable_map.insert(name.clone(), value);
                self.variable_memory_locations.insert(name.clone(), self.instruction_index);
                self.instructions.push(value as Instruction);
                self.instruction_index += 1;
                continue;
            }

            // Handle labels
            if line.ends_with(':') {
                let label = line.trim().trim_end_matches(':').to_string();
                self.variable_memory_locations.insert(label, self.instruction_index);
                continue;
            }

            let tokens = Self::tokenize_instruction(&line, &self.mnemonic_map);
            if tokens.is_empty() { continue; }

            let instr = self.assemble_instruction(&tokens);
            if ["JMP", "JZ", "JGZ", "JLZ", "JS", "JP", "JO"].contains(&tokens[0].as_str()) {
                self.jump_references.insert(self.instruction_index, tokens[1].clone());
            }

            self.instructions.push(instr);
            self.source_to_instruction_index.insert(source_line_num, self.instruction_index);
            self.instruction_index += 1;
        }
    }

    fn assemble_instruction(&mut self, tokens: &[String]) -> Instruction {
        let mut instr = self.mnemonic_map[tokens[0].as_str()] << OPCODE_SHIFT;

        match tokens.len() {
            2 => {
                let operand = &tokens[1];

                let mode: u16 = if self.register_map.contains_key(operand.as_str()) {
                    0b01 // Register mode
                // } else if self.io_map.contains_key(operand.as_str()) {
                //     0b10 // IO mode
                } else {
                    0b00 // Memory (assume variable or label)
                };

                instr += mode << MODE_SHIFT;
                instr += self.parse_arg(operand, OPERANDS_WIDTH);
            },
            3 => {
                let mode: u16 = if ["IN", "OUT"].contains(&tokens[0].as_str()) {
                    0b10 // IO mode
                } else {
                    0b01 // Default: register mode
                };

                instr += mode << MODE_SHIFT;
                instr += self.parse_arg(&tokens[1], REGISTER_WIDTH) << REGISTER_WIDTH;
                instr += self.parse_arg(&tokens[2], REGISTER_WIDTH);
            }
            n if n > 3 => panic!("Too many operands: {:?}", tokens),
            _ => (),
        }

        return instr
    }

    fn parse_arg(&mut self, arg: &str, width: u16) -> u16 {
        let clean = arg.trim();

        // 1. Register name
        if let Some(&reg) = self.register_map.get(clean) {
            return reg;
        }

        // 2. Plain numeric value (decimal, hex, binary)
        if clean.starts_with("0X") || clean.starts_with("0B") || clean.parse::<u16>().is_ok(){
            return self.parse_number(clean, width);
        }

        // 3. Treat as variable to be resolved later
        self.variable_references.insert(self.instruction_index, clean.to_string());
        return 0;
    }

    fn parse_number(&self, s: &str, width: u16) -> u16 {
        if s.starts_with("0X") {
            u16::from_str_radix(&s[2..], 16).unwrap_or(0) % (1 << width)
        } else if s.starts_with("0B") {
            u16::from_str_radix(&s[2..], 2).unwrap_or(0) % (1 << width)
        } else {
            s.parse::<u16>().unwrap_or(0) % (1 << width)
        }
    }


    fn resolve_variable_locations(&mut self) {
        for (&line, var) in &self.variable_references {
            if self.jump_references.values().any(|v| v == var) {
                continue;
            }

            let address = *self.variable_memory_locations.get(var).expect("Unknown variable");
            let base = self.instructions[line - 1] >> OPCODE_SHIFT;
            self.instructions[line - 1] = (base << OPCODE_SHIFT) + address as u16;
        }
    }

    fn resolve_jump_locations(&mut self) {
        for (&line, target_label_or_num) in &self.jump_references {
            let resolved_index = if let Ok(jump_to_line_num) = target_label_or_num.parse::<usize>() {
                *self.source_to_instruction_index
                    .get(&jump_to_line_num)
                    .expect("Invalid jump-to line number")
            } else {
                *self.variable_memory_locations
                    .get(target_label_or_num)
                    .expect("Invalid jump label")
            };

            let base = self.instructions[line - 1] >> OPCODE_SHIFT;
            self.instructions[line - 1] = (base << OPCODE_SHIFT) + resolved_index as u16;
        }
    }

    fn tokenize_instruction(line: &str, map: &HashMap<&str, u16>) -> Vec<String> {
        // Strip everything after the first semicolon
        let line = line.split(';').next().unwrap_or("").trim();

        // Tokenize the cleaned line
        let tokens: Vec<String> = line
            .replace(",", "")
            .split_whitespace()
            .map(|s| s.trim().to_string())
            .collect();

        if tokens.is_empty() {
            return vec![];
        }

        if !map.contains_key(tokens[0].as_str()) {
            panic!("Invalid mnemonic: {}", tokens[0]);
        }

        if tokens.len() == 1 && !["NOP", "HLT", "RET", "ESC"].contains(&tokens[0].as_str()) {
            panic!("Not enough arguments for instruction: {}", tokens[0]);
        }

        return tokens
    }


    fn tokenize_variable(line: &str) -> Vec<String> {
        return line.replace("=", "")
            .split_whitespace()
            .map(|s| s.trim().to_string())
            .collect()
    }
}
