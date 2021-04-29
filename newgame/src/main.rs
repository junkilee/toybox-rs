/**
    Steps to creating a new game.
    - get a name, make sure it doesn't clash
    - modify top-level Cargo.toml to include the new package
    - create package
    - modify toybox/Cargo.toml
    - re-generate toybox/src/lib.rs
*/
use std::{error::Error, io::Write};
use std::fs::{read_to_string, create_dir_all};
use std::fs::File;
use subprocess::{Popen, PopenConfig, Redirection};
use toml::{Value};
use serde::{Deserialize, Serialize};

extern crate argparse;
use argparse::{ArgumentParser, Store, StoreTrue};

#[derive(Deserialize, Clone, Serialize)]
struct Games {
    games : Vec<(String, String, String)>
}
#[derive(Deserialize, Clone, Serialize)]
struct TopCargo {
    workspace : Workspace
}


#[derive(Deserialize, Clone, Serialize)]
struct Workspace {
    members: Vec<String>
}

#[derive(Deserialize, Clone, Serialize)]
struct TBCargo {
    package: Package,
    dependencies: Value,
    features: Option<Features>
}
#[derive(Deserialize, Clone, Serialize)]
struct Package {
    name : String, 
    version : String, 
    authors: Vec<String>,
    edition: String,
    publish: Option<bool>
}
#[derive(Deserialize, Clone, Serialize)]
struct Features {
    default : Vec<String>
}

fn add_to_games(game: String, mainclass: String) -> Result<(), String> {
    // Get the existing game list
    let path = "Games.toml";
    let mut games : Games = toml::from_str(&read_to_string(path.clone()).unwrap()).unwrap();
    
    // See if there are any clashes
    let () = {
        for (old_game, _, _) in &games.games {
            if old_game.eq(&game) {
                return Err(format!("{} already exists", game))
            }
        }
    };

    // Add the new game to the game list
    games.games.push((game.to_string(), game.to_string(), mainclass.to_string()));
    let s = toml::to_string(&games).unwrap();
    let mut f = File::create(path.clone()).unwrap();
    match f.write_all(s.as_bytes()) {
        Err(msg) => Err(msg.to_string()),
        _ => {
            println!("Added {} to {}", &game, &path); 
            Ok(())
        }
    }
}

fn remove_from_games(game: String) -> Result<(), String> {
    // Get the existing game list
    let path = "Games.toml";
    let mut games : Games = toml::from_str(&read_to_string(path.clone()).unwrap()).unwrap();
    // Remove from game list
    let () = {
        for i in 0..games.games.len() {
            if games.games[i].0.eq(&game) {
                games.games.remove(i);
                return Ok(())
            }
        }
        Err(format!("Game {} not found in Games.toml", &game))
    }?;
    // write new file
    let s  = toml::to_string(&games).unwrap();
    let mut f : File = File::create(path.clone()).unwrap();
    match f.write_all(s.as_bytes()) {
        Err(msg) => Err(msg.to_string()),
        _ => Ok(())
    }
}

fn add_to_workspace(dir: String) -> Result<(), String> {
    // Get the existing Cargo.toml
    let mut cargo_toml : TopCargo = toml::from_str(&read_to_string("Cargo.toml").unwrap()).unwrap();
    cargo_toml.workspace.members.push(dir);
    let s = toml::to_string(&cargo_toml).unwrap();
    let mut f = File::create("Cargo.toml").unwrap();
    match f.write_all(s.as_bytes()) {
        Err(msg) => Err(msg.to_string()),
        _ => Ok(())
    }
}

fn add_to_dependences(cargo_toml: &mut TBCargo, dir: &String, game: &String) -> Result<(), String> {
    let deps  = cargo_toml.dependencies.clone();
    let new_deps = {
        match deps {
            Value::Table(mut m) => {
                // Create content of new table
                let mut newg : toml::value::Map<String, Value> = toml::value::Map::new();
                newg.insert("path".into(), Value::String(format!("../{}", dir)));
                newg.insert("version".into(), Value::String("*".into()));
                newg.insert("optional".into(), Value::Boolean(true));
                m.insert(game.into(), Value::Table(newg));
                Value::Table(m)
            },
            _ => return Err(cargo_toml.dependencies.to_string())
        } 
    };
    cargo_toml.dependencies = new_deps;
    Ok(())
}

fn add_to_features(cargo_toml: &mut TBCargo, game: &String) -> Result<(), String> {
    let features = cargo_toml.features.clone();
    let new_features = {
        match features {
            Some(mut f) => {
                f.default.push(game.clone());
                Ok(Some(f))
            },
             None => Err("Input does not have a features attribute".to_string())
        }
    }?;
    cargo_toml.features = new_features;
    Ok(())
}

fn add_to_toybox_cargo(dir: String, game: String) -> Result<(), String> {
    // Can't make a struct because the dependencies table entry requires that 
    // we know the key names a priori. We need to add the new game to the 
    // dependencies and features tables.
    let path = ["toybox", "Cargo.toml"].join(&std::path::MAIN_SEPARATOR.to_string());
    let mut cargo_toml: TBCargo = toml::from_str(&read_to_string(path.clone()).unwrap()).unwrap();
    // let mut cargo_toml = read_to_string(&path).unwrap().parse::<Value>().unwrap();
    add_to_dependences(&mut cargo_toml, &dir, &game)?;
    add_to_features(&mut cargo_toml, &game)?;
    let s = toml::to_string(&cargo_toml).unwrap();
    let mut f = File::create(&path).unwrap();
    match f.write_all(s.as_bytes()) {
        Err(msg) => Err(msg.to_string()),
        _ => Ok(())
    }
}

fn create_project_files(game: String, dir: String) -> Result<(), std::io::Error>{
    let mut p = Popen::create(&["cargo", "new", &*dir, "--lib"], PopenConfig {
        stdout: Redirection::Pipe, ..Default::default()
    }).unwrap();
    loop {
        if p.poll().is_some() {
            create_dir_all([dir.clone(), "src".to_string(), "resources".to_string()].join(&std::path::MAIN_SEPARATOR.to_string()))?;
            File::create([dir.clone(), "src".to_string(), "types.rs".to_string()].join(&std::path::MAIN_SEPARATOR.to_string()))?;
            File::create([dir.clone(), "src".into(), format!("{}.rs", game)].join(&std::path::MAIN_SEPARATOR.to_string()))?;
            return Ok(())        
        }
    }
}

fn update_newgame_cargo(dir: String, game: String) -> Result<(), String> {
    // First we need to change the package name
    let path = [dir, "Cargo.toml".into()].join(&std::path::MAIN_SEPARATOR.to_string());
    let mut cargo_toml : TBCargo = toml::from_str(&read_to_string(path.clone()).unwrap()).unwrap();
    let () = { cargo_toml.package.name = game.clone() };

    // Then we need to add universal dependencies
    let deps  = cargo_toml.dependencies.clone();
    let new_deps = {
        match deps {
            Value::Table(mut m) => {
                // Create content of new table
                let mut newdeps : toml::value::Map<String, Value> = toml::value::Map::new();
                newdeps.insert("serde".into(), Value::String("*".into()));
                newdeps.insert("serde_json".into(), Value::String("*".into()));
                newdeps.insert("serde_derive".into(), Value::String("*".into()));
                newdeps.insert("lazy_static".into(), Value::String("*".into()));
                newdeps.insert("schemars".into(), Value::String("*".into()));
                m.insert(game.into(), Value::Table(newdeps));
                Value::Table(m)
            },
            _ => return Err(cargo_toml.dependencies.to_string())
        } 
    };
    cargo_toml.dependencies = new_deps;

    // Just tack the tb depdencies on at the end. 
    let s = toml::to_string(&cargo_toml).unwrap() + "[dependencies.toybox-core]\nversion = \"*\"\npath = \"../core\"";
    let mut f = File::create(path.clone()).unwrap();
    match f.write_all(s.as_bytes()) {
        Err(msg) => Err(msg.to_string()),
        _ => Ok(())
    }
}

fn update_newgame_lib(gamename: String, classname: String) -> Result<(), String> {
    let lib_template : String = include_str!("resources/lib_template.txt").to_string();
    Ok(())
}

fn populate_files(dir : String, game : String, mainclass: String) -> Result<(), String> {
    update_newgame_cargo(dir.clone(), game.clone());

    update_newgame_lib(game.clone(), mainclass.clone());
    Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
    // Do all I/O here
    let mut verbose = false;
    let mut clear = false;
    let mut game_arg = String::new();
    let () = {
        let mut parser = ArgumentParser::new();
        parser.set_description("Add a new rust game to Toybox.");
        parser.refer(&mut verbose)
              .add_option(&["-v", "--verbose"], StoreTrue, "Be verbose");
        parser.refer(&mut clear)
              .add_option(&["-c", "--clear"], StoreTrue, "Clears the game locally. WARNING: This deletes files!!");
        parser.refer(&mut game_arg)
              .add_argument("new_game_name", Store, "The name of the new game.");
        parser.parse_args_or_exit();
    };

    let g = game_arg.clone();
    let dir = if g.starts_with("tb_") { "".to_string() } else { "tb_".to_string() } + &g;
    let game = if g.starts_with("tb_") { g.get(3..).unwrap().to_string() } else { g.clone() };
    // Capitalize the first letter to make the main class
    let mut v : Vec<char> = game.chars().collect();
    v[0] = v[0].to_uppercase().nth(0).unwrap();
    let mainclass : String = v.into_iter().collect();

    // Let the processing begin!

    if clear {
        println!("Clearing game {}...", g.clone());
        remove_from_games(game.clone())?;
        return Ok(());
    }

    add_to_games(game.clone(), mainclass.clone())?;
    add_to_workspace(dir.clone())?;
    create_project_files(game.clone(), dir.clone())?;
    add_to_toybox_cargo(dir.clone(), game.clone())?;
    populate_files(dir.clone(), game.clone(), mainclass.clone())?;

    println!("Successfully created {}!", game);
    Ok(())
}
