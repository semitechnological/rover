import AppKit
import Foundation
import RoverCore

/// Emoji search — activated by `:` prefix.
@MainActor
public final class EmojiPlugin: RoverPlugin {
    public let id = "builtin.emoji"
    public let name = "Emoji"

    // (character, keywords) — first keyword is used as the subtitle label.
    private static let emojis: [(String, [String])] = [
        // MARK: Faces / Emotions
        ("😀", ["grinning", "happy", "smile", "face", "joy"]),
        ("😂", ["joy", "laughing", "tears", "funny", "lol", "haha"]),
        ("🥹", ["holding back tears", "touched", "grateful", "moved", "emotional"]),
        ("😊", ["blush", "smile", "happy", "pleased", "warm"]),
        ("😎", ["cool", "sunglasses", "awesome", "confident", "smug"]),
        ("🤔", ["thinking", "hmm", "wonder", "ponder", "curious"]),
        ("😴", ["sleeping", "tired", "zzz", "asleep", "bored"]),
        ("😤", ["frustrated", "huffy", "determined", "steam", "annoyed"]),
        ("🥺", ["pleading", "puppy eyes", "sad", "beg", "please"]),
        ("😱", ["scream", "shocked", "horror", "scared", "omg"]),
        ("😍", ["heart eyes", "love", "adore", "crush", "smitten"]),
        ("🤩", ["star struck", "starstruck", "amazed", "dazzled", "wow"]),
        ("😭", ["crying", "sob", "weep", "sad", "devastated"]),
        ("😅", ["sweat smile", "nervous", "awkward", "phew", "relieved"]),
        ("🙃", ["upside down", "sarcastic", "ironic", "silly", "playful"]),
        ("🫡", ["salute", "respect", "yes sir", "roger", "acknowledged"]),
        ("😇", ["angel", "halo", "innocent", "good", "blessed"]),
        ("🤯", ["mind blown", "exploding head", "shocked", "amazed", "whoa"]),
        ("🥳", ["party face", "celebrate", "birthday", "festive", "woohoo"]),
        ("😬", ["grimace", "awkward", "nervous", "cringe", "oops"]),
        ("🫠", ["melting", "awkward", "embarrassed", "nervous", "dissolving"]),
        ("😐", ["neutral", "meh", "blank", "expressionless", "indifferent"]),
        ("🤫", ["shush", "quiet", "secret", "whisper", "silent"]),
        ("😏", ["smirk", "sly", "knowing", "smug", "cheeky"]),
        ("😒", ["unamused", "side eye", "bored", "unimpressed", "whatever"]),

        // MARK: Hands / Gestures
        ("👋", ["wave", "hello", "hi", "goodbye", "bye"]),
        ("🤝", ["handshake", "deal", "agree", "partnership", "greet"]),
        ("👍", ["thumbs up", "like", "approve", "good", "yes", "ok"]),
        ("👎", ["thumbs down", "dislike", "no", "bad", "disapprove"]),
        ("✌️", ["peace", "victory", "two", "v sign", "yes"]),
        ("🤞", ["crossed fingers", "luck", "hope", "wish", "fingers crossed"]),
        ("👌", ["ok", "perfect", "okay", "nice", "chef kiss"]),
        ("🫶", ["heart hands", "love", "care", "affection", "warm"]),
        ("🙌", ["raising hands", "celebrate", "praise", "yes", "hooray"]),
        ("👏", ["clap", "applause", "bravo", "clapping", "praise"]),
        ("🤜", ["right fist", "fist bump", "punch", "solidarity", "bump"]),
        ("🤛", ["left fist", "fist bump", "punch", "solidarity", "bump"]),
        ("🫰", ["snap", "finger snap", "click", "fingers", "magic"]),
        ("👉", ["point right", "right", "direction", "this", "here"]),
        ("👈", ["point left", "left", "direction", "that", "back"]),
        ("☝️", ["point up", "one", "first", "attention", "important"]),
        ("🤙", ["call me", "hang loose", "shaka", "phone", "aloha"]),
        ("💪", ["flex", "muscle", "strong", "bicep", "power"]),
        ("🦾", ["mechanical arm", "prosthetic", "strong", "robot", "cyborg"]),
        ("🖐️", ["raised hand", "high five", "stop", "five", "hello"]),

        // MARK: People
        ("👶", ["baby", "infant", "newborn", "child", "young"]),
        ("🧒", ["child", "kid", "young", "boy", "girl"]),
        ("👦", ["boy", "male", "child", "kid", "young"]),
        ("👧", ["girl", "female", "child", "kid", "young"]),
        ("🧑", ["person", "adult", "human", "individual", "neutral"]),
        ("👨", ["man", "male", "adult", "guy", "person"]),
        ("👩", ["woman", "female", "adult", "lady", "person"]),
        ("🧓", ["older person", "elderly", "senior", "aged", "grandparent"]),
        ("👴", ["old man", "grandfather", "elder", "senior", "grandpa"]),
        ("👵", ["old woman", "grandmother", "elder", "senior", "grandma"]),
        ("🧙", ["mage", "wizard", "witch", "magic", "sorcerer"]),
        ("🧑‍💻", ["technologist", "developer", "coder", "programmer", "hacker", "dev"]),
        ("👨‍🍳", ["male cook", "chef", "cook", "kitchen", "food"]),
        ("👩‍🎨", ["female artist", "artist", "painter", "creative", "art"]),
        ("🧑‍🚀", ["astronaut", "space", "rocket", "cosmonaut", "nasa"]),
        ("👮", ["police", "cop", "officer", "law", "security"]),
        ("🕵️", ["detective", "spy", "investigate", "sleuth", "mystery"]),
        ("🧑‍⚕️", ["health worker", "doctor", "nurse", "medical", "healthcare"]),

        // MARK: Animals
        ("🐶", ["dog", "puppy", "canine", "pet", "woof"]),
        ("🐱", ["cat", "kitten", "feline", "pet", "meow"]),
        ("🐭", ["mouse", "rat", "rodent", "small", "squeaky"]),
        ("🐹", ["hamster", "rodent", "pet", "cute", "fluffy"]),
        ("🐰", ["rabbit", "bunny", "hare", "easter", "cute"]),
        ("🦊", ["fox", "foxy", "cunning", "orange", "wild"]),
        ("🐻", ["bear", "grizzly", "brown bear", "animal", "wild"]),
        ("🐼", ["panda", "china", "bamboo", "bear", "cute"]),
        ("🐨", ["koala", "australia", "marsupial", "cute", "eucalyptus"]),
        ("🐯", ["tiger", "striped", "fierce", "cat", "wild"]),
        ("🦁", ["lion", "king", "pride", "roar", "mane"]),
        ("🐸", ["frog", "toad", "green", "amphibian", "leap"]),
        ("🐵", ["monkey", "primate", "ape", "cheeky", "banana"]),
        ("🐔", ["chicken", "hen", "poultry", "farm", "cluck"]),
        ("🐧", ["penguin", "bird", "arctic", "cute", "waddle"]),
        ("🐦", ["bird", "tweet", "feather", "fly", "avian"]),
        ("🦄", ["unicorn", "magic", "rainbow", "fantasy", "rare"]),
        ("🐍", ["snake", "reptile", "python", "slither", "venom"]),
        ("🦋", ["butterfly", "beautiful", "transform", "wings", "flutter"]),
        ("🐝", ["bee", "honey", "buzz", "insect", "sting"]),

        // MARK: Food
        ("🍎", ["apple", "red", "fruit", "healthy", "food"]),
        ("🍊", ["orange", "citrus", "fruit", "tangerine", "vitamin c"]),
        ("🍋", ["lemon", "citrus", "sour", "yellow", "fruit"]),
        ("🍇", ["grapes", "wine", "fruit", "purple", "vineyard"]),
        ("🍓", ["strawberry", "fruit", "red", "sweet", "berry"]),
        ("🍑", ["peach", "fruit", "fuzzy", "sweet", "summer"]),
        ("🍕", ["pizza", "italian", "cheese", "slice", "food"]),
        ("🍔", ["burger", "hamburger", "fast food", "beef", "sandwich"]),
        ("🌮", ["taco", "mexican", "food", "shell", "spicy"]),
        ("🍜", ["noodle", "ramen", "soup", "asian", "bowl"]),
        ("🍣", ["sushi", "japanese", "fish", "rice", "raw"]),
        ("🍦", ["soft serve", "ice cream", "dessert", "sweet", "cold"]),
        ("🎂", ["birthday cake", "cake", "celebration", "candles", "bake"]),
        ("🍩", ["donut", "doughnut", "sweet", "ring", "glazed"]),
        ("🍪", ["cookie", "biscuit", "sweet", "baked", "chocolate chip"]),
        ("☕️", ["coffee", "hot", "morning", "caffeine", "espresso"]),
        ("🧃", ["juice box", "drink", "beverage", "kids", "straw"]),
        ("🍺", ["beer", "ale", "drink", "pub", "cold"]),
        ("🥂", ["champagne", "toast", "celebrate", "sparkling", "cheers"]),
        ("🧁", ["cupcake", "muffin", "sweet", "bake", "frosting"]),

        // MARK: Nature
        ("🌸", ["cherry blossom", "spring", "flower", "pink", "japan"]),
        ("🌺", ["hibiscus", "tropical", "flower", "red", "bloom"]),
        ("🌻", ["sunflower", "flower", "yellow", "sunny", "summer"]),
        ("🌹", ["rose", "flower", "love", "red", "romantic"]),
        ("🌷", ["tulip", "flower", "spring", "pink", "garden"]),
        ("🍀", ["four leaf clover", "luck", "lucky", "green", "shamrock"]),
        ("🌿", ["herb", "leaf", "plant", "green", "nature"]),
        ("🌱", ["seedling", "grow", "plant", "sprout", "new"]),
        ("🌲", ["evergreen tree", "pine", "forest", "christmas", "nature"]),
        ("🌳", ["deciduous tree", "tree", "oak", "nature", "shade"]),
        ("⛰️", ["mountain", "peak", "summit", "nature", "outdoors"]),
        ("🏔️", ["snow mountain", "snowy peak", "alps", "winter", "climb"]),
        ("🌊", ["wave", "ocean", "sea", "surf", "water"]),
        ("🌈", ["rainbow", "colorful", "hope", "lgbtq", "pride"]),
        ("⭐️", ["star", "bright", "favorite", "rating", "night"]),
        ("🌙", ["moon", "night", "crescent", "sleep", "lunar"]),
        ("☀️", ["sun", "sunny", "bright", "day", "warm"]),
        ("❄️", ["snowflake", "cold", "winter", "ice", "frozen"]),
        ("🔥", ["fire", "hot", "flame", "lit", "trending"]),
        ("💧", ["droplet", "water", "drop", "rain", "liquid"]),

        // MARK: Objects
        ("💻", ["laptop", "computer", "mac", "code", "dev", "work"]),
        ("📱", ["phone", "mobile", "smartphone", "iphone", "cell"]),
        ("⌨️", ["keyboard", "type", "input", "keys", "typing"]),
        ("🖥️", ["desktop", "monitor", "screen", "computer", "display"]),
        ("📷", ["camera", "photo", "picture", "snapshot", "shoot"]),
        ("🎵", ["music note", "music", "song", "melody", "audio"]),
        ("🎮", ["game controller", "gaming", "play", "video game", "gamer"]),
        ("📚", ["books", "reading", "study", "library", "learn"]),
        ("✏️", ["pencil", "write", "draw", "edit", "sketch"]),
        ("📝", ["memo", "note", "write", "list", "document"]),
        ("🔑", ["key", "unlock", "access", "password", "lock"]),
        ("💡", ["light bulb", "idea", "bright", "thought", "insight"]),
        ("🔧", ["wrench", "tool", "fix", "repair", "build"]),
        ("🔨", ["hammer", "tool", "build", "nail", "construct"]),
        ("🪄", ["magic wand", "magic", "trick", "wizard", "poof"]),
        ("🎁", ["gift", "present", "birthday", "surprise", "wrap"]),
        ("📦", ["package", "box", "delivery", "ship", "parcel"]),
        ("🗂️", ["card index dividers", "folder", "file", "organize", "tabs"]),
        ("🧲", ["magnet", "attract", "pull", "north", "south"]),
        ("🔋", ["battery", "charge", "power", "energy", "low"]),

        // MARK: Transport
        ("🚀", ["rocket", "launch", "space", "fast", "startup"]),
        ("✈️", ["airplane", "plane", "fly", "travel", "flight"]),
        ("🚂", ["locomotive", "train", "rail", "steam", "engine"]),
        ("🚗", ["car", "drive", "vehicle", "auto", "road"]),
        ("🚕", ["taxi", "cab", "yellow", "ride", "fare"]),
        ("🚌", ["bus", "public transport", "route", "commute", "stop"]),
        ("🛸", ["flying saucer", "ufo", "alien", "sci-fi", "space"]),
        ("🚁", ["helicopter", "chopper", "fly", "hover", "rotor"]),
        ("⛵", ["sailboat", "sail", "wind", "sea", "boat"]),
        ("🚲", ["bicycle", "bike", "cycle", "ride", "pedal"]),
        ("🛵", ["motor scooter", "scooter", "moped", "ride", "city"]),
        ("🚢", ["ship", "cruise", "boat", "ocean liner", "sea"]),
        ("🛩️", ["small airplane", "plane", "propeller", "fly", "light aircraft"]),
        ("🚄", ["high speed train", "bullet train", "shinkansen", "fast", "rail"]),
        ("🏎️", ["racing car", "race", "speed", "formula", "fast"]),
        ("🚑", ["ambulance", "emergency", "hospital", "medical", "ems"]),
        ("🚒", ["fire engine", "firefighter", "truck", "emergency", "red"]),
        ("🚓", ["police car", "cop", "law", "emergency", "siren"]),
        ("🛻", ["pickup truck", "truck", "offroad", "vehicle", "haul"]),
        ("🚐", ["minibus", "van", "shuttle", "transport", "group"]),

        // MARK: Symbols
        ("❤️", ["red heart", "love", "heart", "romance", "affection"]),
        ("🧡", ["orange heart", "orange", "warm", "care", "heart"]),
        ("💛", ["yellow heart", "yellow", "happy", "sunshine", "heart"]),
        ("💚", ["green heart", "green", "nature", "health", "heart"]),
        ("💙", ["blue heart", "blue", "calm", "trust", "heart"]),
        ("💜", ["purple heart", "purple", "royalty", "heart", "military"]),
        ("🖤", ["black heart", "black", "dark", "goth", "heart"]),
        ("🤍", ["white heart", "white", "pure", "clean", "heart"]),
        ("💔", ["broken heart", "heartbreak", "sad", "love lost", "breakup"]),
        ("✅", ["check mark", "done", "complete", "yes", "tick", "correct"]),
        ("❌", ["cross mark", "no", "wrong", "error", "cancel", "x"]),
        ("⭕", ["hollow red circle", "correct", "yes", "circle", "ok"]),
        ("💯", ["hundred points", "100", "perfect", "score", "full marks"]),
        ("🔴", ["red circle", "red", "stop", "danger", "record"]),
        ("🟡", ["yellow circle", "yellow", "caution", "warning", "pending"]),
        ("🟢", ["green circle", "green", "go", "ok", "active"]),
        ("🔵", ["blue circle", "blue", "info", "circle", "cool"]),
        ("⚡", ["lightning", "electric", "fast", "power", "zap", "flash"]),
        ("🌀", ["cyclone", "swirl", "spin", "tornado", "vortex"]),
        ("♾️", ["infinity", "forever", "loop", "unlimited", "infinite"]),

        // MARK: Activities
        ("⚽", ["soccer", "football", "ball", "sport", "kick"]),
        ("🏀", ["basketball", "hoop", "sport", "ball", "nba"]),
        ("🎾", ["tennis", "racket", "ball", "sport", "court"]),
        ("🏈", ["american football", "nfl", "sport", "ball", "gridiron"]),
        ("⚾", ["baseball", "mlb", "sport", "ball", "pitch"]),
        ("🎱", ["billiards", "pool", "eight ball", "snooker", "cue"]),
        ("🏆", ["trophy", "winner", "champion", "award", "gold"]),
        ("🥇", ["gold medal", "first", "winner", "champion", "gold"]),
        ("🎯", ["bullseye", "target", "accurate", "goal", "aim"]),
        ("🎲", ["dice", "game", "random", "roll", "luck"]),
        ("🎭", ["performing arts", "theater", "drama", "masks", "acting"]),
        ("🎨", ["artist palette", "art", "paint", "color", "creative"]),
        ("🎬", ["clapper board", "film", "movie", "action", "cinema"]),
        ("🎤", ["microphone", "sing", "vocal", "karaoke", "perform"]),
        ("🎧", ["headphone", "music", "listen", "audio", "podcast"]),
        ("🎹", ["musical keyboard", "piano", "keys", "instrument", "music"]),
        ("🎸", ["guitar", "rock", "music", "strings", "electric"]),
        ("🏋️", ["weight lifter", "gym", "lift", "fitness", "strong"]),
        ("🧘", ["lotus", "meditation", "yoga", "calm", "mindful"]),
        ("✨", ["sparkles", "magic", "shine", "glitter", "stars"]),
    ]

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        guard query.hasPrefix(":") else { return [] }
        let term = query.dropFirst().trimmingCharacters(in: .whitespaces).lowercased()
        if term.isEmpty {
            return Self.emojis.prefix(8).map { makeItem($0.0, $0.1[0]) }
        }
        return Self.emojis.compactMap { emoji, keywords in
            let score: Double
            if keywords.contains(where: { $0 == term }) {
                score = 1.0
            } else if keywords.contains(where: { $0.hasPrefix(term) }) {
                score = 0.85
            } else if keywords.contains(where: { $0.contains(term) }) {
                score = 0.7
            } else {
                return nil
            }
            return PickerItem(
                id: "emoji.\(emoji)",
                pluginID: id,
                title: emoji,
                subtitle: keywords[0],
                iconName: nil,
                score: score
            )
        }
        .sorted { $0.score > $1.score }
        .prefix(20)
        .map { $0 }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        // Extract the emoji character from the item ID ("emoji.<char>")
        let emoji: String
        if itemID.hasPrefix("emoji.") {
            emoji = String(itemID.dropFirst("emoji.".count))
        } else {
            emoji = itemID
        }
        return .copyToClipboard(emoji)
    }

    private func makeItem(_ char: String, _ label: String) -> PickerItem {
        PickerItem(
            id: "emoji.\(char)",
            pluginID: id,
            title: char,
            subtitle: label,
            iconName: nil,
            score: 1.0
        )
    }
}
