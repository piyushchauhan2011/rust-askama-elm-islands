port module Admin exposing (main)

import Browser
import File exposing (File)
import Html exposing (Html, a, button, code, div, figcaption, figure, form, h1, h2, header, img, input, label, option, p, section, select, small, span, strong, table, tbody, td, text, textarea, th, thead, tr)
import Html.Attributes as Attr exposing (accept, alt, attribute, class, disabled, for, height, href, id, name, placeholder, rel, required, rows, selected, src, step, target, type_, value, width)
import Html.Events exposing (on, onClick, onInput, preventDefaultOn)
import Html.Keyed
import Http
import Json.Decode as Decode
import Json.Encode as Encode
import Task


port pushUrl : String -> Cmd msg


port urlChanged : (String -> msg) -> Sub msg


type alias Flags =
    { path : String }


type alias Model =
    { path : String
    , error : String
    , notice : String
    , email : String
    , password : String
    , pending : Bool
    , dashboard : Maybe Dashboard
    , inquiries : List Inquiry
    , rows : List Row
    , editor : Maybe Editor
    , media : List Media
    , assetId : Maybe String
    , upload : Maybe File
    , uploadPreview : String
    , uploadAlt : String
    , uploadCaption : String
    , uploadGeneration : Int
    , token : String
    , pendingBody : Maybe Encode.Value
    , pendingPath : String
    }


type alias Dashboard =
    { destinations : Int
    , hotels : Int
    , rooms : Int
    , offers : Int
    , inquiries : Int
    , posts : Int
    , recent : List Inquiry
    , media : List Media
    }


type alias Inquiry =
    { id : String
    , name : String
    , email : String
    , status : String
    , hotelName : String
    , checkIn : String
    , checkOut : String
    }


type alias Row =
    { id : String
    , label : String
    , slug : String
    , status : String
    , hotelName : String
    }


type alias Editor =
    { kind : String
    , record : Record
    , blocks : List Block
    , destinations : List Choice
    , hotels : List Choice
    , media : List Media
    , fieldErrors : List ( String, String )
    , imageTarget : Maybe String
    }


type alias Record =
    { id : String
    , name : String
    , title : String
    , slug : String
    , status : String
    , country : String
    , eyebrow : String
    , destinationId : String
    , propertyType : String
    , address : String
    , rating : String
    , priceFrom : String
    , currency : String
    , latitude : String
    , longitude : String
    , hotelId : String
    , maxGuests : String
    , sizeSqm : String
    , bed : String
    , discountPercent : String
    , validFrom : String
    , validTo : String
    , author : String
    , summary : String
    , description : String
    , excerpt : String
    , terms : String
    , heroImage : String
    , image : String
    , seoTitle : String
    , seoDescription : String
    }


type Block
    = Paragraph String
    | Heading Int String
    | Blockquote String
    | BulletList (List String)
    | OrderedList (List String)
    | ImageBlock String String
    | TableBlock (List (List String))
    | HotelEmbed String
    | Preserved Encode.Value


type BlockChange
    = Text String
    | Level String
    | AltText String
    | ListItem Int String
    | AddItem
    | RemoveItem Int
    | Cell Int Int String
    | AddRow
    | AddColumn


type alias Choice =
    { id : String
    , name : String
    }


type alias Media =
    { id : String
    , filename : String
    , alt : String
    , caption : String
    , width : Int
    , height : Int
    , variants : Variants
    }


type alias Variants =
    { thumbnail : String
    , small : String
    , medium : String
    , large : String
    , original : String
    }


type Msg
    = Navigate String
    | UrlChanged String
    | Edit String String
    | SubmitLogin
    | GotLogin (Result String ())
    | Logout
    | GotDashboard (Result String Dashboard)
    | GotInquiries (Result String (List Inquiry))
    | SetStatus String String
    | SetPublish String String
    | GotPublish (Result String ( String, String ))
    | GotRows (Result String (List Row))
    | GotEditor (Result String Editor)
    | EditField String String
    | ChangeBlock Int BlockChange
    | AddBlock String
    | RemoveBlock Int
    | MoveBlock Int Int
    | ChooseImage String
    | ClosePicker
    | UseImage String
    | SaveEditor
    | GotSave (Result String String)
    | GotFile File
    | GotPreview (Result Never String)
    | EditAlt String
    | EditCaption String
    | UploadMedia
    | Uploaded (Result String ())
    | OpenAsset String
    | CloseAsset
    | GotCsrf (Result String String)
    | Noop


main : Program Flags Model Msg
main =
    Browser.element
        { init = init
        , view = view
        , update = update
        , subscriptions = \_ -> urlChanged UrlChanged
        }


init : Flags -> ( Model, Cmd Msg )
init flags =
    load (blank flags.path)


blank : String -> Model
blank path =
    { path = path
    , error = ""
    , notice = ""
    , email = ""
    , password = ""
    , pending = False
    , dashboard = Nothing
    , inquiries = []
    , rows = []
    , editor = Nothing
    , media = []
    , assetId = Nothing
    , upload = Nothing
    , uploadPreview = ""
    , uploadAlt = ""
    , uploadCaption = ""
    , uploadGeneration = 0
    , token = ""
    , pendingBody = Nothing
    , pendingPath = ""
    }


load : Model -> ( Model, Cmd Msg )
load model =
    let
        parts =
            String.split "/" model.path |> List.filter ((/=) "")
    in
    case parts of
        [ "admin", "login" ] ->
            ( model, Cmd.none )

        [ "admin" ] ->
            ( model, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        [ "admin", "inquiries" ] ->
            ( model, getJson "/api/admin/inquiries" (Decode.list inquiryDecoder) GotInquiries )

        [ "admin", "media" ] ->
            ( model, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        [ "admin", kind ] ->
            if List.member kind kinds then
                ( model, getJson ("/api/admin/" ++ kind) (Decode.list rowDecoder) GotRows )

            else
                ( { model | error = "Section not found." }, Cmd.none )

        [ "admin", kind, id ] ->
            if List.member kind kinds then
                ( model, getJson ("/api/admin/" ++ kind ++ "/" ++ id) editorDecoder GotEditor )

            else
                ( { model | error = "Section not found." }, Cmd.none )

        _ ->
            ( model, Cmd.none )


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Navigate path ->
            load { model | path = path, error = "", notice = "", assetId = Nothing }

                |> Tuple.mapSecond (\cmd -> Cmd.batch [ pushUrl path, cmd ])

        UrlChanged path ->
            load { model | path = path, error = "", assetId = Nothing }

        Edit "email" next ->
            ( { model | email = next }, Cmd.none )

        Edit "password" next ->
            ( { model | password = next }, Cmd.none )

        Edit _ _ ->
            ( model, Cmd.none )

        SubmitLogin ->
            ( { model | pending = True, error = "", pendingPath = "/api/admin/login", pendingBody = Just (Encode.object [ ( "email", Encode.string model.email ), ( "password", Encode.string model.password ) ]) }, fetchCsrf )

        GotLogin (Ok ()) ->
            load { model | path = "/admin", pending = False, error = "" }
                |> Tuple.mapSecond (\cmd -> Cmd.batch [ pushUrl "/admin", cmd ])

        GotLogin (Err reason) ->
            ( { model | pending = False, error = reason }, Cmd.none )

        Logout ->
            ( { model | pendingPath = "/api/admin/logout", pendingBody = Just (Encode.object []) }, fetchCsrf )

        GotDashboard (Ok dashboard) ->
            ( { model | dashboard = Just dashboard, media = dashboard.media }, Cmd.none )

        GotDashboard (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotInquiries (Ok items) ->
            ( { model | inquiries = items }, Cmd.none )

        GotInquiries (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        SetStatus id status ->
            ( { model | pendingPath = "/api/admin/inquiries/" ++ id ++ "/status", pendingBody = Just (Encode.object [ ( "status", Encode.string status ) ]) }, fetchCsrf )

        SetPublish id status ->
            let
                kind =
                    sectionOf model.path
            in
            ( { model | pendingPath = "/api/admin/" ++ kind ++ "/" ++ id ++ "/publish", pendingBody = Just (Encode.object [ ( "status", Encode.string status ) ]) }, fetchCsrf )

        GotPublish (Ok ( id, status )) ->
            ( { model | rows = List.map (\row -> if row.id == id then { row | status = status } else row) model.rows }, Cmd.none )

        GotPublish (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotRows (Ok rows) ->
            ( { model | rows = rows }, Cmd.none )

        GotRows (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotEditor (Ok editor) ->
            ( { model | editor = Just { editor | kind = sectionOf model.path }, media = editor.media }, Cmd.none )

        GotEditor (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        EditField key next ->
            ( { model | editor = Maybe.map (\editor -> { editor | record = setField key next editor.record }) model.editor }, Cmd.none )

        ChangeBlock index change ->
            ( { model | editor = Maybe.map (\editor -> { editor | blocks = List.indexedMap (\position block -> if position == index then changeBlock change block else block) editor.blocks }) model.editor }, Cmd.none )

        AddBlock kind ->
            ( { model | editor = Maybe.map (\editor -> { editor | blocks = editor.blocks ++ [ freshBlock kind ] }) model.editor }, Cmd.none )

        RemoveBlock index ->
            ( { model | editor = Maybe.map (\editor -> { editor | blocks = List.indexedMap Tuple.pair editor.blocks |> List.filterMap (\( position, block ) -> if position == index then Nothing else Just block) }) model.editor }, Cmd.none )

        MoveBlock index delta ->
            ( { model | editor = Maybe.map (\editor -> { editor | blocks = moveBlock index delta editor.blocks }) model.editor }, Cmd.none )

        ChooseImage target ->
            ( { model | editor = Maybe.map (\editor -> { editor | imageTarget = Just target }) model.editor }, Cmd.none )

        ClosePicker ->
            ( { model | editor = Maybe.map (\editor -> { editor | imageTarget = Nothing }) model.editor }, Cmd.none )

        UseImage url ->
            ( { model | editor = Maybe.map (applyImage url) model.editor }, Cmd.none )

        SaveEditor ->
            case model.editor of
                Just editor ->
                    ( { model | pending = True, error = "", pendingPath = "/api/admin/" ++ editor.kind, pendingBody = Just (editorBody editor) }, fetchCsrf )

                Nothing ->
                    ( model, Cmd.none )

        GotSave (Ok _) ->
            case model.editor of
                Just editor ->
                    let
                        path =
                            "/admin/" ++ editor.kind

                        ( loaded, cmd ) =
                            load { model | path = path, error = "", pending = False, editor = Nothing }
                    in
                    ( { loaded | notice = "Saved." }, Cmd.batch [ pushUrl path, cmd ] )

                Nothing ->
                    ( { model | notice = "Saved.", pending = False }, Cmd.none )

        GotSave (Err reason) ->
            ( applySaveError reason model, Cmd.none )

        GotFile file ->
            if File.size file > 10 * 1024 * 1024 then
                ( { model | error = "Images must be no larger than 10 MB.", upload = Nothing, uploadPreview = "" }, Cmd.none )

            else
                ( { model | upload = Just file, error = "" }, Task.attempt GotPreview (File.toUrl file) )

        GotPreview (Ok url) ->
            ( { model | uploadPreview = url }, Cmd.none )

        GotPreview (Err _) ->
            ( model, Cmd.none )

        EditAlt next ->
            ( { model | uploadAlt = next }, Cmd.none )

        EditCaption next ->
            ( { model | uploadCaption = next }, Cmd.none )

        UploadMedia ->
            if model.upload == Nothing then
                ( { model | error = "Choose an image file." }, Cmd.none )

            else if String.length (String.trim model.uploadAlt) < 2 then
                ( { model | error = "Alternative text is required." }, Cmd.none )

            else
                ( { model | pending = True, error = "", pendingPath = "media", pendingBody = Nothing }, fetchCsrf )

        Uploaded (Ok ()) ->
            ( { model | pending = False, notice = "Image uploaded.", upload = Nothing, uploadPreview = "", uploadAlt = "", uploadCaption = "", uploadGeneration = model.uploadGeneration + 1 }, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        Uploaded (Err reason) ->
            ( { model | pending = False, error = readableError reason }, Cmd.none )

        OpenAsset id ->
            ( { model | assetId = Just id }, Cmd.none )

        CloseAsset ->
            ( { model | assetId = Nothing }, Cmd.none )

        GotCsrf (Ok token) ->
            sendPending { model | token = token }

        GotCsrf (Err reason) ->
            ( { model | error = reason, pending = False }, Cmd.none )

        Noop ->
            ( model, Cmd.none )


sendPending : Model -> ( Model, Cmd Msg )
sendPending model =
    case ( model.pendingPath, model.pendingBody, model.upload ) of
        ( "/api/admin/login", Just body, _ ) ->
            ( { model | pendingBody = Nothing }, postJson model.token "/api/admin/login" body (Decode.succeed ()) GotLogin )

        ( "media", _, Just file ) ->
            ( { model | upload = Nothing }, uploadFile model.token file model.uploadAlt model.uploadCaption )

        ( "/api/admin/logout", _, _ ) ->
            ( model, postJson model.token "/api/admin/logout" (Encode.object []) (Decode.succeed ()) (\_ -> Navigate "/admin/login") )

        ( path, Just body, _ ) ->
            if String.endsWith "/status" path then
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.succeed "") (\_ -> Navigate model.path) )

            else if String.contains "/publish" path then
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.map2 Tuple.pair (Decode.field "id" Decode.string) (Decode.field "status" Decode.string)) GotPublish )

            else
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.oneOf [ Decode.field "id" Decode.string, Decode.succeed "ok" ]) GotSave )

        _ ->
            ( model, Cmd.none )


view : Model -> Html Msg
view model =
    if model.path == "/admin/login" then
        loginView model

    else
        shell model


loginView : Model -> Html Msg
loginView model =
    div [ class "admin-login" ]
        [ div [ class "card" ]
            [ div [ class "admin-login__header" ]
                [ p [ class "eyebrow" ] [ text "ELSEWHERE ADMIN" ]
                , h1 [ class "admin-login__title" ] [ text "Welcome back." ]
                ]
            , div [ class "card-content" ]
                [ form [ prevent SubmitLogin ]
                    [ field "admin-email" "Email" model.email "email"
                    , field "admin-password" "Password" model.password "password"
                    , button [ class "button is-primary", type_ "submit", disabled model.pending ] [ text "Sign in" ]
                    , errorText model.error
                    ]
                ]
            ]
        ]


shell : Model -> Html Msg
shell model =
    let
        section =
            sectionOf model.path
    in
    div [ class "admin-shell" ]
        [ div [ class "admin-shell__layout" ]
            [ Html.aside [ class "admin-shell__sidebar" ]
                [ div [ class "admin-shell__sidebar-inner" ]
                    [ div [ class "admin-shell__brand" ]
                        [ a [ href "/", attribute "data-native" "true" ] [ text "Elsewhere" ]
                        , p [] [ text "Admin studio" ]
                        ]
                    , Html.nav [ class "admin-shell__nav", attribute "aria-label" "Admin navigation" ]
                        (List.map (navItem model.path) nav)
                    , div [ class "admin-shell__signout" ]
                        [ button [ class "button is-ghost", type_ "button", onClick Logout ] [ text "Sign out" ] ]
                    ]
                ]
            , div [ class "admin-shell__main" ]
                [ Html.header [ class "admin-shell__header" ]
                    [ p [ class "eyebrow" ] [ text "ELSEWHERE ADMIN" ]
                    , h1 [] [ text (title section model.path) ]
                    ]
                , errorText model.error
                , if model.notice /= "" then
                    p [ class "notification is-success" ] [ text model.notice ]

                  else
                    text ""
                , pageBody section model
                ]
            ]
        ]


pageBody : String -> Model -> Html Msg
pageBody section model =
    case section of
        "overview" ->
            dashboardView model.dashboard

        "inquiries" ->
            inquirySection model.inquiries

        "media" ->
            mediaView model

        _ ->
            if List.any (\kind -> String.contains ("/admin/" ++ kind ++ "/") model.path) kinds then
                editorView model

            else
                listing section model.rows


dashboardView : Maybe Dashboard -> Html Msg
dashboardView dashboard =
    case dashboard of
        Nothing ->
            p [] [ text "Loading the studio…" ]

        Just data ->
            div []
                [ div [ class "admin-dashboard__stats" ]
                    [ stat "Destinations" data.destinations
                    , stat "Hotels" data.hotels
                    , stat "Rooms" data.rooms
                    , stat "Offers" data.offers
                    , stat "Journal" data.posts
                    , stat "Inquiries" data.inquiries
                    ]
                , sectionCard "RECENT" "Recent inquiries" (text "") (inquiryTable data.recent)
                ]


inquirySection : List Inquiry -> Html Msg
inquirySection items =
    sectionCard "INBOX" "Inquiries" (text "") (inquiryTable items)


sectionCard : String -> String -> Html Msg -> Html Msg -> Html Msg
sectionCard eyebrow titleText action body =
    section [ class "card admin-dashboard__section" ]
        [ header [ class "admin-dashboard__section-header" ]
            [ div []
                [ p [ class "eyebrow" ] [ text eyebrow ]
                , h2 [ class "admin-dashboard__section-title" ] [ text titleText ]
                ]
            , action
            ]
        , div [ class "card-content" ] [ body ]
        ]


inquiryTable : List Inquiry -> Html Msg
inquiryTable items =
    if List.isEmpty items then
        p [ class "admin-dashboard__empty" ] [ text "No inquiries yet." ]

    else
        div [ class "table-container" ]
            [ table [ class "table is-fullwidth" ]
                [ thead []
                    [ tr []
                        [ th [ attribute "scope" "col" ] [ text "Guest" ]
                        , th [ attribute "scope" "col" ] [ text "Hotel" ]
                        , th [ attribute "scope" "col" ] [ text "Dates" ]
                        , th [ attribute "scope" "col" ] [ text "Status" ]
                        ]
                    ]
                , tbody []
                    (List.map
                        (\item ->
                            tr []
                                [ td []
                                    [ strong [] [ text item.name ]
                                    , small [] [ text item.email ]
                                    ]
                                , td [] [ text item.hotelName ]
                                , td []
                                    [ text item.checkIn
                                    , Html.br [] []
                                    , text item.checkOut
                                    ]
                                , td [] [ statusSelect item.status [ "new", "contacted", "closed" ] (SetStatus item.id) ]
                                ]
                        )
                        items
                    )
                ]
            ]


listing : String -> List Row -> Html Msg
listing kind rows =
    let
        newPath =
            "/admin/" ++ kind ++ "/new"
    in
    sectionCard "CONTENT"
        (plural kind)
        (a [ class "button is-primary is-outlined", href newPath, onNavigate newPath ] [ text ("New " ++ singular kind) ])
        (if List.isEmpty rows then
            p [ class "admin-dashboard__empty" ] [ text "No records yet." ]

         else
            div [ class "table-container" ]
                [ table [ class "table is-fullwidth" ]
                    [ thead []
                        [ tr []
                            [ th [ attribute "scope" "col" ] [ text "Name" ]
                            , if kind == "rooms" || kind == "offers" then
                                th [ attribute "scope" "col" ] [ text "Hotel" ]

                              else
                                text ""
                            , th [ attribute "scope" "col" ] [ text "Slug" ]
                            , th [ attribute "scope" "col" ] [ text "Publication" ]
                            , th [ attribute "scope" "col", class "has-text-right" ] [ text "Actions" ]
                            ]
                        ]
                    , tbody [] (List.map (listingRow kind) rows)
                    ]
                ]
        )


listingRow : String -> Row -> Html Msg
listingRow kind row =
    let
        editPath =
            "/admin/" ++ kind ++ "/" ++ row.id
    in
    tr []
        [ td [] [ strong [] [ text row.label ] ]
        , if kind == "rooms" || kind == "offers" then
            td [] [ text row.hotelName ]

          else
            text ""
        , td [] [ code [] [ text ("/" ++ row.slug) ] ]
        , td [] [ statusSelect row.status [ "draft", "published" ] (SetPublish row.id) ]
        , td [ class "has-text-right" ]
            [ a [ class "button is-ghost is-small", href editPath, onNavigate editPath ] [ text "Edit" ] ]
        ]


statusSelect : String -> List String -> (String -> Msg) -> Html Msg
statusSelect current options toMsg =
    div [ class "select" ]
        [ select [ class "admin-dashboard__status", onInput toMsg ]
            (List.map (\optionValue -> option [ value optionValue, selected (optionValue == current) ] [ text (capitalize optionValue) ]) options)
        ]


editorView : Model -> Html Msg
editorView model =
    case model.editor of
        Nothing ->
            p [] [ text "Loading editor…" ]

        Just editor ->
            let
                listPath =
                    "/admin/" ++ editor.kind
            in
            div [ class "admin-content-form" ]
                [ a [ class "button is-ghost admin-content-form__back", href listPath, onNavigate listPath ] [ text ("Back to " ++ plural editor.kind) ]
                , form [ class "admin-content-form__form", prevent SaveEditor ]
                    [ coreCard editor
                    , copyCard editor
                    , if editor.kind == "destinations" || editor.kind == "posts" then
                        richCard editor

                      else
                        text ""
                    , if editor.kind == "destinations" || editor.kind == "hotels" || editor.kind == "posts" then
                        seoCard editor

                      else
                        text ""
                    , if List.isEmpty editor.fieldErrors then
                        text ""

                      else
                        div [ class "notification is-danger", attribute "role" "alert" ] [ text "Check the highlighted fields and save again." ]
                    , div [ class "admin-content-form__actions" ]
                        [ a [ class "button is-primary is-outlined", href listPath, onNavigate listPath ] [ text "Cancel" ]
                        , button [ class "button is-primary", type_ "submit", disabled model.pending ] [ text (if model.pending then "Saving…" else "Save " ++ singular editor.kind) ]
                        ]
                    ]
                , imagePicker editor
                ]


coreCard : Editor -> Html Msg
coreCard editor =
    let
        record =
            editor.record
    in
    formCard "Core details"
        (div [ class "card-content admin-content-form__grid" ]
            ((if editor.kind == "posts" || editor.kind == "offers" then
                [ textInput editor "title" "Title" record.title True "" ]

              else
                [ textInput editor "name" "Name" record.name True "" ]
             )
                ++ [ textInput editor "slug" "Slug" record.slug True "[a-z0-9]+(?:-[a-z0-9]+)*"
                   , selectInput editor "status" "Publication status" record.status [ { id = "draft", name = "Draft" }, { id = "published", name = "Published" } ]
                   ]
                ++ coreExtras editor
            )
        )


coreExtras : Editor -> List (Html Msg)
coreExtras editor =
    let
        record =
            editor.record
    in
    case editor.kind of
        "destinations" ->
            [ textInput editor "country" "Country" record.country True ""
            , textInput editor "eyebrow" "Eyebrow" record.eyebrow True ""
            ]

        "hotels" ->
            [ selectInput editor "destinationId" "Destination" record.destinationId editor.destinations
            , textInput editor "propertyType" "Property type" record.propertyType True ""
            , textInput editor "address" "Address" record.address True "wide"
            , numberInput editor "rating" "Rating" record.rating "0.1" "0" "5"
            , numberInput editor "priceFrom" "Price from" record.priceFrom "1" "0" ""
            , textInput editor "currency" "Currency" record.currency True ""
            , numberInput editor "latitude" "Latitude" record.latitude "any" "" ""
            , numberInput editor "longitude" "Longitude" record.longitude "any" "" ""
            ]

        "rooms" ->
            [ selectInput editor "hotelId" "Hotel" record.hotelId editor.hotels
            , numberInput editor "priceFrom" "Price from" record.priceFrom "1" "0" ""
            , numberInput editor "maxGuests" "Maximum guests" record.maxGuests "1" "1" ""
            , numberInput editor "sizeSqm" "Size (m²)" record.sizeSqm "1" "1" ""
            , textInput editor "bed" "Bed" record.bed True ""
            ]

        "offers" ->
            [ selectInput editor "hotelId" "Hotel" record.hotelId editor.hotels
            , numberInput editor "discountPercent" "Discount percent" record.discountPercent "1" "0" "100"
            , dateInput editor "validFrom" "Valid from" record.validFrom
            , dateInput editor "validTo" "Valid to" record.validTo
            ]

        "posts" ->
            [ textInput editor "author" "Author" record.author True "" ]

        _ ->
            []


copyCard : Editor -> Html Msg
copyCard editor =
    let
        record =
            editor.record
    in
    let
        copy =
            case editor.kind of
                "posts" ->
                    [ areaInput editor "excerpt" "Excerpt" record.excerpt ]

                "hotels" ->
                    [ areaInput editor "summary" "Summary" record.summary
                    , areaInput editor "description" "Description" record.description
                    ]

                "offers" ->
                    [ areaInput editor "summary" "Summary" record.summary
                    , areaInput editor "terms" "Terms" record.terms
                    ]

                _ ->
                    [ areaInput editor "summary" "Summary" record.summary ]
    in
    formCard "Copy & media"
        (div [ class "card-content admin-content-form__stack" ] (copy ++ [ imageField editor ]))


imageField : Editor -> Html Msg
imageField editor =
    let
        key =
            if editor.kind == "rooms" || editor.kind == "offers" then
                "image"

            else
                "heroImage"

        current =
            if key == "image" then
                editor.record.image

            else
                editor.record.heroImage
    in
    div [ class "admin-content-form__field" ]
        [ label [ class "label", for key ] [ text "Image" ]
        , input [ class "input", id key, name key, value current, onInput (EditField key) ] []
        , button [ class "button is-ghost is-small", type_ "button", onClick (ChooseImage key) ] [ text "Choose image" ]
        , if current == "" then
            text ""

          else
            img [ class "media-library__preview", src current, alt "", width 160 ] []
        , fieldError editor key
        ]


seoCard : Editor -> Html Msg
seoCard editor =
    formCard "Search metadata"
        (div [ class "card-content admin-content-form__stack" ]
            [ textInput editor "seoTitle" "SEO title" editor.record.seoTitle True ""
            , areaInput editor "seoDescription" "SEO description" editor.record.seoDescription
            ]
        )


richCard : Editor -> Html Msg
richCard editor =
    formCard "Rich content"
        (div [ class "card-content" ]
            [ div [ class "rich-text-editor" ]
                [ div [ class "rich-text-editor__toolbar" ]
                    [ div [ class "rich-text-editor__group" ]
                        [ span [ class "rich-text-editor__group-label" ] [ text "Add block" ]
                        , div [ class "rich-text-editor__group-controls" ]
                            (List.map
                                (\( labelText, kind ) -> button [ class "button rich-text-editor__tool", type_ "button", onClick (AddBlock kind) ] [ text labelText ])
                                [ ( "Paragraph", "paragraph" ), ( "Heading", "heading" ), ( "Quote", "blockquote" ), ( "Bullets", "bulletList" ), ( "Numbers", "orderedList" ), ( "Image", "image" ), ( "Table", "table" ), ( "Hotel", "hotelEmbed" ) ]
                            )
                        ]
                    ]
                , div [ class "rich-text-editor__canvas" ]
                    [ div [ class "rich-text-editor__content", attribute "aria-label" "Rich content editor" ]
                        (if List.isEmpty editor.blocks then
                            [ p [ class "rich-text-editor__empty" ] [ text "No blocks yet. Add one from the toolbar." ] ]

                         else
                            List.indexedMap (blockView editor) editor.blocks
                        )
                    ]
                ]
            ]
        )


blockView : Editor -> Int -> Block -> Html Msg
blockView editor index block =
    div [ class "field" ]
        [ div [ class "admin-dashboard__section-header" ]
            [ strong [] [ text (blockLabel block) ]
            , div []
                [ button [ class "button is-ghost is-small", type_ "button", onClick (MoveBlock index -1) ] [ text "Up" ]
                , button [ class "button is-ghost is-small", type_ "button", onClick (MoveBlock index 1) ] [ text "Down" ]
                , button [ class "button is-ghost is-small", type_ "button", onClick (RemoveBlock index) ] [ text "Remove" ]
                ]
            ]
        , blockFields editor index block
        ]


blockFields : Editor -> Int -> Block -> Html Msg
blockFields editor index block =
    case block of
        Paragraph current ->
            textarea [ class "textarea", rows 4, value current, onInput (\next -> ChangeBlock index (Text next)) ] []

        Heading level current ->
            div [ class "admin-content-form__stack" ]
                [ select [ onInput (\next -> ChangeBlock index (Level next)) ]
                    (List.map (\choice -> option [ value (String.fromInt choice), selected (choice == level) ] [ text ("Heading " ++ String.fromInt choice) ]) [ 2, 3, 4 ])
                , textarea [ class "textarea", rows 2, value current, onInput (\next -> ChangeBlock index (Text next)) ] []
                ]

        Blockquote current ->
            textarea [ class "textarea", rows 3, value current, onInput (\next -> ChangeBlock index (Text next)) ] []

        BulletList items ->
            listFields index items

        OrderedList items ->
            listFields index items

        ImageBlock imageSrc imageAlt ->
            div [ class "admin-content-form__stack" ]
                [ input [ class "input", value imageSrc, onInput (\next -> ChangeBlock index (Text next)) ] []
                , input [ class "input", placeholder "Alternative text", value imageAlt, onInput (\next -> ChangeBlock index (AltText next)) ] []
                , button [ class "button is-ghost is-small", type_ "button", onClick (ChooseImage ("block:" ++ String.fromInt index)) ] [ text "Choose image" ]
                ]

        TableBlock tableRows ->
            div []
                [ table [ class "table is-fullwidth" ]
                    [ tbody []
                        (List.indexedMap
                            (\rowIndex cells ->
                                tr []
                                    (List.indexedMap
                                        (\columnIndex cell ->
                                            td [] [ input [ class "input", value cell, onInput (\next -> ChangeBlock index (Cell rowIndex columnIndex next)) ] [] ]
                                        )
                                        cells
                                    )
                            )
                            tableRows
                        )
                    ]
                , button [ class "button is-ghost is-small", type_ "button", onClick (ChangeBlock index AddRow) ] [ text "Add row" ]
                , button [ class "button is-ghost is-small", type_ "button", onClick (ChangeBlock index AddColumn) ] [ text "Add column" ]
                ]

        HotelEmbed entityId ->
            select [ onInput (\next -> ChangeBlock index (Text next)) ]
                (option [ value "", selected (entityId == "") ] [ text "Select a hotel" ]
                    :: List.map (\hotel -> option [ value hotel.id, selected (hotel.id == entityId) ] [ text hotel.name ]) editor.hotels
                )

        Preserved raw ->
            p [ class "rich-text-editor__empty" ] [ text ("Preserved " ++ preservedType raw ++ " block. It will be saved unchanged.") ]


listFields : Int -> List String -> Html Msg
listFields index items =
    div [ class "admin-content-form__stack" ]
        (List.indexedMap
            (\itemIndex item ->
                div [ class "admin-content-form__actions" ]
                    [ input [ class "input", value item, onInput (\next -> ChangeBlock index (ListItem itemIndex next)) ] []
                    , button [ class "button is-ghost is-small", type_ "button", onClick (ChangeBlock index (RemoveItem itemIndex)) ] [ text "Remove" ]
                    ]
            )
            items
            ++ [ button [ class "button is-ghost is-small", type_ "button", onClick (ChangeBlock index AddItem) ] [ text "Add item" ] ]
        )


imagePicker : Editor -> Html Msg
imagePicker editor =
    case editor.imageTarget of
        Nothing ->
            text ""

        Just _ ->
            Html.node "dialog"
                [ class "modal media-library__modal"
                , attribute "data-open" "true"
                , attribute "aria-labelledby" "image-picker-title"
                , preventDefaultOn "cancel" (Decode.succeed ( ClosePicker, True ))
                ]
                [ div [ class "media-library__modal-content" ]
                    [ header [ class "media-library__modal-header" ]
                        [ h2 [ id "image-picker-title" ] [ text "Choose an image" ]
                        , button [ class "button is-ghost", type_ "button", onClick ClosePicker ] [ text "Close" ]
                        ]
                    , div [ class "media-library__modal-body" ]
                        [ if List.isEmpty editor.media then
                            p [ class "admin-dashboard__empty" ] [ text "No uploaded media yet. Add an image in the media library first." ]

                          else
                            div [ class "media-library__grid" ] (List.map pickerAsset editor.media)
                        ]
                    ]
                ]


pickerAsset : Media -> Html Msg
pickerAsset item =
    button [ class "media-library__asset", type_ "button", onClick (UseImage (preferredUrl item)) ]
        [ div [ class "media-library__asset-image" ]
            [ img [ src item.variants.thumbnail, alt item.alt, width 240 ] []
            ]
        , div [ class "media-library__asset-copy" ]
            [ strong [] [ text item.filename ]
            , small [] [ text item.alt ]
            ]
        ]


mediaView : Model -> Html Msg
mediaView model =
    div [ class "media-library" ]
        [ form [ class "media-library__upload", prevent UploadMedia ]
            [ label [ class "media-library__dropzone" ]
                [ if model.uploadPreview /= "" then
                    img [ class "media-library__preview", src model.uploadPreview, alt "Selected upload preview" ] []

                  else
                    span [ class "media-library__prompt" ] [ text "Choose image" ]
                , Html.Keyed.node "span"
                    []
                    [ ( String.fromInt model.uploadGeneration, input [ class "is-sr-only", type_ "file", accept "image/jpeg,image/png,image/webp,image/avif", name "file", required True, onFile GotFile ] [] )
                    ]
                ]
            , div [ class "media-library__fields" ]
                [ div [ class "field" ]
                    [ label [ class "label", for "media-alt" ] [ text "Alternative text" ]
                    , input [ class "input", id "media-alt", name "alt", placeholder "Describe what is visible", required True, value model.uploadAlt, onInput EditAlt ] []
                    ]
                , div [ class "field media-library__caption-field" ]
                    [ label [ class "label", for "media-caption" ] [ text "Caption (optional)" ]
                    , textarea [ class "textarea", id "media-caption", name "caption", rows 2, placeholder "Additional context or photo credit", value model.uploadCaption, onInput EditCaption ] []
                    ]
                , small [ class "media-library__help" ] [ text "JPEG, PNG, WebP, or AVIF · maximum 10 MB. Responsive WebP variants are generated automatically." ]
                ]
            , button [ class "button is-primary", type_ "submit", disabled (model.pending || model.upload == Nothing) ] [ text (if model.pending then "Processing…" else "Upload image") ]
            ]
        , if List.isEmpty model.media then
            div [ class "media-library__empty" ]
                [ div []
                    [ p [] [ text "No uploaded media yet" ]
                    , small [] [ text "Choose an image above to create responsive variants." ]
                    ]
                ]

          else
            div [ class "media-library__grid" ] (List.map (assetButton model.assetId) model.media)
        , assetDialog model
        ]


assetButton : Maybe String -> Media -> Html Msg
assetButton _ item =
    button [ class "media-library__asset", type_ "button", onClick (OpenAsset item.id) ]
        [ div [ class "media-library__asset-image" ]
            [ img [ src item.variants.thumbnail, alt item.alt, width 240, height (scaledHeight 240 item) ] []
            ]
        , div [ class "media-library__asset-copy" ]
            [ strong [] [ text item.filename ]
            , small [] [ text (String.fromInt item.width ++ " × " ++ String.fromInt item.height ++ " · 5 variants") ]
            ]
        ]


assetDialog : Model -> Html Msg
assetDialog model =
    case selectedAsset model of
        Nothing ->
            text ""

        Just item ->
            Html.node "dialog"
                [ class "modal media-library__modal"
                , attribute "data-open" "true"
                , attribute "aria-labelledby" "media-library-title"
                , preventDefaultOn "cancel" (Decode.succeed ( CloseAsset, True ))
                ]
                [ div [ class "media-library__modal-content" ]
                    [ header [ class "media-library__modal-header" ]
                        [ div []
                            [ h2 [ id "media-library-title" ] [ text item.filename ]
                            , p [ id "media-library-description", class "media-library__description" ] [ text (item.alt ++ " · " ++ String.fromInt item.width ++ " × " ++ String.fromInt item.height ++ " original") ]
                            ]
                        , button [ class "button is-ghost", type_ "button", attribute "aria-label" "Close media details", onClick CloseAsset ] [ text "Close" ]
                        ]
                    , div [ class "media-library__modal-body" ]
                        [ div [ class "media-library__variants" ]
                            (List.filterMap (variantFigure item) [ ( "thumbnail", 160 ), ( "small", 480 ), ( "medium", 960 ), ( "large", 1600 ), ( "original", item.width ) ])
                        ]
                    ]
                ]


variantFigure : Media -> ( String, Int ) -> Maybe (Html Msg)
variantFigure item ( variantName, maxWidth ) =
    let
        url =
            variantUrl item variantName
    in
    if url == "" then
        Nothing

    else
        let
            displayWidth =
                Basics.min maxWidth item.width

            displayHeight =
                scaledHeight displayWidth item
        in
        Just
            (figure [ class ("media-library__variant" ++ (if variantName == "original" then " media-library__variant--original" else "")) ]
                [ div [ class "media-library__variant-image" ] [ img [ src url, alt item.alt ] [] ]
                , figcaption []
                    [ span []
                        [ strong [] [ text (capitalize variantName) ]
                        , text " "
                        , span [] [ text (String.fromInt displayWidth ++ " × " ++ String.fromInt displayHeight) ]
                        ]
                    , a [ class "button is-ghost button-xs", href url, target "_blank", rel "noreferrer" ] [ text "Open" ]
                    ]
                ]
            )


formCard : String -> Html Msg -> Html Msg
formCard titleText body =
    section [ class "card" ]
        [ header [ class "admin-content-form__card-header" ]
            [ h2 [ class "admin-content-form__card-title" ] [ text titleText ]
            ]
        , body
        ]


textInput : Editor -> String -> String -> String -> Bool -> String -> Html Msg
textInput editor key labelText current isRequired wide =
    div [ class ("admin-content-form__field" ++ (if wide == "wide" then " admin-content-form__field--wide" else "")) ]
        [ label [ class "label", for key ] [ text labelText ]
        , input
            ([ class "input", id key, name key, value current, onInput (EditField key) ]
                ++ (if isRequired then
                        [ required True ]

                    else
                        []
                   )
                ++ (if wide /= "" && wide /= "wide" then
                        [ attribute "pattern" wide ]

                    else
                        []
                   )
            )
            []
        , fieldError editor key
        ]


numberInput : Editor -> String -> String -> String -> String -> String -> String -> Html Msg
numberInput editor key labelText current stepValue minValue maxValue =
    div [ class "admin-content-form__field" ]
        [ label [ class "label", for key ] [ text labelText ]
        , input
            ([ class "input", id key, name key, type_ "number", step stepValue, value current, onInput (EditField key) ]
                ++ (if minValue /= "" then
                        [ Attr.min minValue ]

                    else
                        []
                   )
                ++ (if maxValue /= "" then
                        [ Attr.max maxValue ]

                    else
                        []
                   )
            )
            []
        , fieldError editor key
        ]


dateInput : Editor -> String -> String -> String -> Html Msg
dateInput editor key labelText current =
    div [ class "admin-content-form__field" ]
        [ label [ class "label", for key ] [ text labelText ]
        , input [ class "input", id key, name key, type_ "date", value current, onInput (EditField key) ] []
        , fieldError editor key
        ]


areaInput : Editor -> String -> String -> String -> Html Msg
areaInput editor key labelText current =
    div [ class "admin-content-form__field" ]
        [ label [ class "label", for key ] [ text labelText ]
        , textarea [ class "textarea", id key, name key, rows 5, value current, onInput (EditField key) ] []
        , fieldError editor key
        ]


selectInput : Editor -> String -> String -> String -> List Choice -> Html Msg
selectInput editor key labelText current choices =
    div [ class "admin-content-form__field" ]
        [ label [ class "label", for key ] [ text labelText ]
        , div [ class "select is-fullwidth" ]
            [ select [ id key, name key, required True, onInput (EditField key) ]
                (option [ value "", selected (current == ""), disabled True ] [ text "Select…" ]
                    :: List.map (\choice -> option [ value choice.id, selected (choice.id == current) ] [ text choice.name ]) choices
                )
            ]
        , fieldError editor key
        ]


fieldError : Editor -> String -> Html msg
fieldError editor key =
    case List.filter (\( name, _ ) -> name == key) editor.fieldErrors |> List.head of
        Just ( _, message ) ->
            p [ class "help is-danger" ] [ text message ]

        Nothing ->
            text ""


stat : String -> Int -> Html msg
stat labelText count =
    div [ class "card" ]
        [ div [ class "card-content" ]
            [ span [] [ text labelText ]
            , strong [] [ text (String.fromInt count) ]
            ]
        ]


nav : List ( String, String )
nav =
    [ ( "Overview", "/admin" )
    , ( "Inquiries", "/admin/inquiries" )
    , ( "Hotels", "/admin/hotels" )
    , ( "Rooms", "/admin/rooms" )
    , ( "Offers", "/admin/offers" )
    , ( "Destinations", "/admin/destinations" )
    , ( "Journal", "/admin/posts" )
    , ( "Media", "/admin/media" )
    ]


kinds : List String
kinds =
    [ "destinations", "hotels", "rooms", "offers", "posts" ]


navItem : String -> ( String, String ) -> Html Msg
navItem current ( labelText, path ) =
    let
        here =
            current == path || (path /= "/admin" && String.startsWith (path ++ "/") current)
    in
    a
        (href path
            :: onNavigate path
            :: (if here then
                    [ attribute "aria-current" "page" ]

                else
                    []
               )
        )
        [ text labelText ]


title : String -> String -> String
title section path =
    let
        parts =
            String.split "/" path |> List.filter ((/=) "")
    in
    if List.length parts > 2 && String.endsWith "/new" path then
        "New " ++ singular section

    else if List.length parts > 2 then
        "Edit " ++ singular section

    else
        plural section


plural : String -> String
plural kind =
    case kind of
        "overview" ->
            "Overview"

        "inquiries" ->
            "Inquiries"

        "posts" ->
            "Journal"

        "media" ->
            "Media library"

        other ->
            capitalize other


singular : String -> String
singular kind =
    case kind of
        "destinations" ->
            "destination"

        "hotels" ->
            "hotel"

        "rooms" ->
            "room"

        "offers" ->
            "offer"

        "posts" ->
            "post"

        other ->
            other


sectionOf : String -> String
sectionOf path =
    String.split "/" path |> List.filter ((/=) "") |> List.drop 1 |> List.head |> Maybe.withDefault "overview"


field : String -> String -> String -> String -> Html Msg
field fieldId labelText current kind =
    div [ class "field" ]
        [ label [ class "label", for fieldId ] [ text labelText ]
        , input [ class "input", id fieldId, type_ kind, value current, onInput (Edit (if kind == "password" then "password" else "email")) ] []
        ]


errorText : String -> Html msg
errorText message =
    if message == "" then
        text ""

    else
        p [ class "notification is-danger", attribute "role" "alert" ] [ text message ]


onNavigate : String -> Html.Attribute Msg
onNavigate path =
    preventDefaultOn "click" (Decode.succeed ( Navigate path, True ))


prevent : Msg -> Html.Attribute Msg
prevent msg =
    preventDefaultOn "submit" (Decode.succeed ( msg, True ))


onFile : (File -> Msg) -> Html.Attribute Msg
onFile toMsg =
    on "change" (Decode.at [ "target", "files" ] (Decode.index 0 File.decoder) |> Decode.map toMsg)


fetchCsrf : Cmd Msg
fetchCsrf =
    Http.get
        { url = "/api/csrf"
        , expect = expectJson (Decode.field "csrfToken" Decode.string) GotCsrf
        }


getJson : String -> Decode.Decoder a -> (Result String a -> Msg) -> Cmd Msg
getJson url decoder toMsg =
    Http.get { url = url, expect = expectJson decoder toMsg }


postJson : String -> String -> Encode.Value -> Decode.Decoder a -> (Result String a -> Msg) -> Cmd Msg
postJson token url body decoder toMsg =
    Http.request
        { method = "POST"
        , headers = [ Http.header "X-CSRF-Token" token, Http.header "Content-Type" "application/json" ]
        , url = url
        , body = Http.jsonBody body
        , expect = expectJson decoder toMsg
        , timeout = Nothing
        , tracker = Nothing
        }


expectJson : Decode.Decoder a -> (Result String a -> Msg) -> Http.Expect Msg
expectJson decoder toMsg =
    Http.expectStringResponse toMsg <|
        \response ->
            case response of
                Http.GoodStatus_ _ body ->
                    Decode.decodeString decoder body |> Result.mapError Decode.errorToString

                Http.BadStatus_ metadata body ->
                    if metadata.statusCode == 401 && not (String.contains "/login" metadata.url) then
                        Err "Session expired. Sign in again."

                    else
                        Err body

                _ ->
                    Err "Request failed."


uploadFile : String -> File -> String -> String -> Cmd Msg
uploadFile token file altText caption =
    Http.request
        { method = "POST"
        , headers = [ Http.header "X-CSRF-Token" token ]
        , url = "/api/admin/media"
        , body = Http.multipartBody [ Http.filePart "file" file, Http.stringPart "alt" altText, Http.stringPart "caption" caption ]
        , expect = expectJson (Decode.succeed ()) Uploaded
        , timeout = Nothing
        , tracker = Nothing
        }


dashboardDecoder : Decode.Decoder Dashboard
dashboardDecoder =
    Decode.map8 Dashboard
        (Decode.field "destinationCount" Decode.int)
        (Decode.field "hotelCount" Decode.int)
        (Decode.field "roomCount" Decode.int)
        (Decode.field "offerCount" Decode.int)
        (Decode.field "inquiryCount" Decode.int)
        (Decode.field "postCount" Decode.int)
        (Decode.field "recentInquiries" (Decode.list inquiryDecoder))
        (Decode.field "media" (Decode.list mediaDecoder))


mediaDecoder : Decode.Decoder Media
mediaDecoder =
    Decode.map7 Media
        (Decode.field "id" Decode.string)
        (Decode.field "filename" Decode.string)
        (Decode.field "alt" Decode.string)
        (Decode.oneOf [ Decode.field "caption" Decode.string, Decode.succeed "" ])
        (Decode.field "width" Decode.int)
        (Decode.field "height" Decode.int)
        (Decode.field "variants" variantsDecoder)


variantsDecoder : Decode.Decoder Variants
variantsDecoder =
    Decode.map5 Variants
        (variant "thumbnail")
        (variant "small")
        (variant "medium")
        (variant "large")
        (variant "original")


variant : String -> Decode.Decoder String
variant name =
    Decode.oneOf [ Decode.field name Decode.string, Decode.succeed "" ]


inquiryDecoder : Decode.Decoder Inquiry
inquiryDecoder =
    Decode.map7 Inquiry
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "email" Decode.string)
        (Decode.field "status" Decode.string)
        (Decode.oneOf [ Decode.field "hotelName" Decode.string, Decode.succeed "" ])
        (Decode.oneOf [ Decode.field "checkIn" Decode.string, Decode.succeed "" ])
        (Decode.oneOf [ Decode.field "checkOut" Decode.string, Decode.succeed "" ])


rowDecoder : Decode.Decoder Row
rowDecoder =
    Decode.map5 Row
        (Decode.field "id" Decode.string)
        (Decode.field "label" Decode.string)
        (Decode.field "slug" Decode.string)
        (Decode.field "status" Decode.string)
        (Decode.oneOf [ Decode.field "hotelName" Decode.string, Decode.succeed "" ])


editorDecoder : Decode.Decoder Editor
editorDecoder =
    Decode.map5 editorFrom
        (Decode.field "item" recordDecoder)
        (Decode.oneOf [ Decode.at [ "item", "content" ] (Decode.list blockDecoder), Decode.succeed [] ])
        (Decode.field "destinations" (Decode.list choiceDecoder))
        (Decode.field "hotels" (Decode.list choiceDecoder))
        (Decode.oneOf [ Decode.field "media" (Decode.list mediaDecoder), Decode.succeed [] ])


editorFrom : Record -> List Block -> List Choice -> List Choice -> List Media -> Editor
editorFrom record blocks destinations hotels media =
    { kind = "", record = record, blocks = blocks, destinations = destinations, hotels = hotels, media = media, fieldErrors = [], imageTarget = Nothing }


recordDecoder : Decode.Decoder Record
recordDecoder =
    Decode.succeed Record
        |> andMap (textField "id")
        |> andMap (textField "name")
        |> andMap (textField "title")
        |> andMap (textField "slug")
        |> andMap (textField "status")
        |> andMap (textField "country")
        |> andMap (textField "eyebrow")
        |> andMap (textField "destinationId")
        |> andMap (textField "propertyType")
        |> andMap (textField "address")
        |> andMap (textField "rating")
        |> andMap (textField "priceFrom")
        |> andMap (textField "currency")
        |> andMap (textField "latitude")
        |> andMap (textField "longitude")
        |> andMap (textField "hotelId")
        |> andMap (textField "maxGuests")
        |> andMap (textField "sizeSqm")
        |> andMap (textField "bed")
        |> andMap (textField "discountPercent")
        |> andMap (textField "validFrom")
        |> andMap (textField "validTo")
        |> andMap (textField "author")
        |> andMap (textField "summary")
        |> andMap (textField "description")
        |> andMap (textField "excerpt")
        |> andMap (textField "terms")
        |> andMap (textField "heroImage")
        |> andMap (textField "image")
        |> andMap (textField "seoTitle")
        |> andMap (textField "seoDescription")


andMap : Decode.Decoder a -> Decode.Decoder (a -> b) -> Decode.Decoder b
andMap =
    Decode.map2 (|>)


textField : String -> Decode.Decoder String
textField key =
    Decode.oneOf
        [ Decode.field key Decode.string
        , Decode.field key (Decode.map String.fromInt Decode.int)
        , Decode.field key (Decode.map String.fromFloat Decode.float)
        , Decode.succeed ""
        ]


blockDecoder : Decode.Decoder Block
blockDecoder =
    Decode.value
        |> Decode.andThen
            (\raw ->
                case Decode.decodeValue (Decode.field "type" Decode.string) raw of
                    Ok "paragraph" ->
                        decodeBlock (Decode.map Paragraph (plain "text")) raw

                    Ok "heading" ->
                        decodeBlock (Decode.map2 Heading (Decode.oneOf [ Decode.field "level" Decode.int, Decode.succeed 2 ]) (plain "text")) raw

                    Ok "blockquote" ->
                        decodeBlock (Decode.map Blockquote (plain "text")) raw

                    Ok "bulletList" ->
                        decodeBlock (Decode.map BulletList (Decode.oneOf [ Decode.field "items" (Decode.list Decode.string), Decode.succeed [] ])) raw

                    Ok "orderedList" ->
                        decodeBlock (Decode.map OrderedList (Decode.oneOf [ Decode.field "items" (Decode.list Decode.string), Decode.succeed [] ])) raw

                    Ok "image" ->
                        decodeBlock (Decode.map2 ImageBlock (plain "src") (plain "alt")) raw

                    Ok "table" ->
                        decodeBlock (Decode.map TableBlock (Decode.oneOf [ Decode.field "rows" (Decode.list (Decode.list Decode.string)), Decode.succeed [] ])) raw

                    Ok "hotelEmbed" ->
                        decodeBlock (Decode.map HotelEmbed (plain "entityId")) raw

                    _ ->
                        Decode.succeed (Preserved raw)
            )


decodeBlock : Decode.Decoder Block -> Encode.Value -> Decode.Decoder Block
decodeBlock decoder raw =
    case Decode.decodeValue decoder raw of
        Ok block ->
            Decode.succeed block

        Err _ ->
            Decode.succeed (Preserved raw)


plain : String -> Decode.Decoder String
plain key =
    Decode.oneOf [ Decode.field key Decode.string, Decode.succeed "" ]


choiceDecoder : Decode.Decoder Choice
choiceDecoder =
    Decode.map2 Choice (Decode.field "id" Decode.string) (Decode.field "name" Decode.string)


setField : String -> String -> Record -> Record
setField key next record =
    case key of
        "name" ->
            { record | name = next }

        "title" ->
            { record | title = next }

        "slug" ->
            { record | slug = next }

        "status" ->
            { record | status = next }

        "country" ->
            { record | country = next }

        "eyebrow" ->
            { record | eyebrow = next }

        "destinationId" ->
            { record | destinationId = next }

        "propertyType" ->
            { record | propertyType = next }

        "address" ->
            { record | address = next }

        "rating" ->
            { record | rating = next }

        "priceFrom" ->
            { record | priceFrom = next }

        "currency" ->
            { record | currency = next }

        "latitude" ->
            { record | latitude = next }

        "longitude" ->
            { record | longitude = next }

        "hotelId" ->
            { record | hotelId = next }

        "maxGuests" ->
            { record | maxGuests = next }

        "sizeSqm" ->
            { record | sizeSqm = next }

        "bed" ->
            { record | bed = next }

        "discountPercent" ->
            { record | discountPercent = next }

        "validFrom" ->
            { record | validFrom = next }

        "validTo" ->
            { record | validTo = next }

        "author" ->
            { record | author = next }

        "summary" ->
            { record | summary = next }

        "description" ->
            { record | description = next }

        "excerpt" ->
            { record | excerpt = next }

        "terms" ->
            { record | terms = next }

        "heroImage" ->
            { record | heroImage = next }

        "image" ->
            { record | image = next }

        "seoTitle" ->
            { record | seoTitle = next }

        "seoDescription" ->
            { record | seoDescription = next }

        _ ->
            record


freshBlock : String -> Block
freshBlock kind =
    case kind of
        "heading" ->
            Heading 2 ""

        "blockquote" ->
            Blockquote ""

        "bulletList" ->
            BulletList [ "" ]

        "orderedList" ->
            OrderedList [ "" ]

        "image" ->
            ImageBlock "" ""

        "table" ->
            TableBlock [ [ "Heading", "Detail" ], [ "", "" ] ]

        "hotelEmbed" ->
            HotelEmbed ""

        _ ->
            Paragraph ""


changeBlock : BlockChange -> Block -> Block
changeBlock change block =
    case ( change, block ) of
        ( Text next, Paragraph _ ) ->
            Paragraph next

        ( Text next, Heading level _ ) ->
            Heading level next

        ( Level next, Heading _ current ) ->
            Heading (String.toInt next |> Maybe.withDefault 2) current

        ( Text next, Blockquote _ ) ->
            Blockquote next

        ( ListItem itemIndex next, BulletList items ) ->
            BulletList (replaceAt itemIndex next items)

        ( ListItem itemIndex next, OrderedList items ) ->
            OrderedList (replaceAt itemIndex next items)

        ( AddItem, BulletList items ) ->
            BulletList (items ++ [ "" ])

        ( AddItem, OrderedList items ) ->
            OrderedList (items ++ [ "" ])

        ( RemoveItem itemIndex, BulletList items ) ->
            BulletList (removeAt itemIndex items)

        ( RemoveItem itemIndex, OrderedList items ) ->
            OrderedList (removeAt itemIndex items)

        ( Text next, ImageBlock _ imageAlt ) ->
            ImageBlock next imageAlt

        ( AltText next, ImageBlock imageSrc _ ) ->
            ImageBlock imageSrc next

        ( Cell rowIndex columnIndex next, TableBlock tableRows ) ->
            TableBlock
                (List.indexedMap
                    (\position cells ->
                        if position == rowIndex then
                            replaceAt columnIndex next cells

                        else
                            cells
                    )
                    tableRows
                )

        ( AddRow, TableBlock tableRows ) ->
            let
                width =
                    List.head tableRows |> Maybe.map List.length |> Maybe.withDefault 2
            in
            TableBlock (tableRows ++ [ List.repeat width "" ])

        ( AddColumn, TableBlock tableRows ) ->
            TableBlock (List.map (\cells -> cells ++ [ "" ]) tableRows)

        ( Text next, HotelEmbed _ ) ->
            HotelEmbed next

        _ ->
            block


applyImage : String -> Editor -> Editor
applyImage url editor =
    case editor.imageTarget of
        Just target ->
            if String.startsWith "block:" target then
                case String.dropLeft 6 target |> String.toInt of
                    Just index ->
                        { editor
                            | imageTarget = Nothing
                            , blocks =
                                List.indexedMap
                                    (\position block ->
                                        if position == index then
                                            case block of
                                                ImageBlock _ imageAlt ->
                                                    ImageBlock url imageAlt

                                                other ->
                                                    other

                                        else
                                            block
                                    )
                                    editor.blocks
                        }

                    Nothing ->
                        { editor | imageTarget = Nothing }

            else
                { editor | imageTarget = Nothing, record = setField target url editor.record }

        Nothing ->
            editor


moveBlock : Int -> Int -> List Block -> List Block
moveBlock index delta blocks =
    let
        target =
            index + delta
    in
    if target < 0 || target >= List.length blocks then
        blocks

    else
        List.indexedMap
            (\position block ->
                if position == index then
                    at target blocks |> Maybe.withDefault block

                else if position == target then
                    at index blocks |> Maybe.withDefault block

                else
                    block
            )
            blocks


editorBody : Editor -> Encode.Value
editorBody editor =
    let
        record =
            editor.record

        text key current =
            ( key, Encode.string current )

        shared =
            [ text "id" record.id, text "slug" record.slug, text "status" (if record.status == "" then "draft" else record.status) ]
    in
    Encode.object
        (shared
            ++ (case editor.kind of
                    "destinations" ->
                        [ text "name" record.name, text "country" record.country, text "summary" record.summary, text "eyebrow" record.eyebrow, text "heroImage" record.heroImage, text "seoTitle" record.seoTitle, text "seoDescription" record.seoDescription, ( "content", Encode.list encodeBlock editor.blocks ) ]

                    "hotels" ->
                        [ text "name" record.name, text "destinationId" record.destinationId, text "summary" record.summary, text "description" record.description, text "address" record.address, text "propertyType" record.propertyType, text "heroImage" record.heroImage, text "currency" record.currency, text "seoTitle" record.seoTitle, text "seoDescription" record.seoDescription, text "priceFrom" record.priceFrom, text "rating" record.rating, text "latitude" record.latitude, text "longitude" record.longitude ]

                    "rooms" ->
                        [ text "name" record.name, text "hotelId" record.hotelId, text "summary" record.summary, text "image" record.image, text "bed" record.bed, text "priceFrom" record.priceFrom, text "maxGuests" record.maxGuests, text "sizeSqm" record.sizeSqm ]

                    "offers" ->
                        [ text "title" record.title, text "hotelId" record.hotelId, text "summary" record.summary, text "image" record.image, text "terms" record.terms, text "validFrom" record.validFrom, text "validTo" record.validTo, text "discountPercent" record.discountPercent ]

                    "posts" ->
                        [ text "title" record.title, text "excerpt" record.excerpt, text "author" record.author, text "heroImage" record.heroImage, text "seoTitle" record.seoTitle, text "seoDescription" record.seoDescription, ( "content", Encode.list encodeBlock editor.blocks ) ]

                    _ ->
                        []
               )
        )


encodeBlock : Block -> Encode.Value
encodeBlock block =
    case block of
        Paragraph current ->
            Encode.object [ ( "type", Encode.string "paragraph" ), ( "text", Encode.string current ) ]

        Heading level current ->
            Encode.object [ ( "type", Encode.string "heading" ), ( "level", Encode.int level ), ( "text", Encode.string current ) ]

        Blockquote current ->
            Encode.object [ ( "type", Encode.string "blockquote" ), ( "text", Encode.string current ) ]

        BulletList items ->
            Encode.object [ ( "type", Encode.string "bulletList" ), ( "items", Encode.list Encode.string items ) ]

        OrderedList items ->
            Encode.object [ ( "type", Encode.string "orderedList" ), ( "items", Encode.list Encode.string items ) ]

        ImageBlock imageSrc imageAlt ->
            Encode.object [ ( "type", Encode.string "image" ), ( "src", Encode.string imageSrc ), ( "alt", Encode.string imageAlt ) ]

        TableBlock tableRows ->
            Encode.object [ ( "type", Encode.string "table" ), ( "rows", Encode.list (Encode.list Encode.string) tableRows ) ]

        HotelEmbed entityId ->
            Encode.object [ ( "type", Encode.string "hotelEmbed" ), ( "entityId", Encode.string entityId ) ]

        Preserved raw ->
            raw


applySaveError : String -> Model -> Model
applySaveError reason model =
    case Decode.decodeString (Decode.field "errors" (Decode.keyValuePairs (Decode.list Decode.string))) reason of
        Ok pairs ->
            { model | pending = False, error = "", editor = Maybe.map (\editor -> { editor | fieldErrors = List.map (\( key, messages ) -> ( key, String.join " " messages )) pairs }) model.editor }

        Err _ ->
            { model | pending = False, error = readableError reason }


readableError : String -> String
readableError reason =
    Decode.decodeString (Decode.field "error" Decode.string) reason |> Result.withDefault reason


selectedAsset : Model -> Maybe Media
selectedAsset model =
    model.assetId |> Maybe.andThen (\assetId -> List.filter (\item -> item.id == assetId) model.media |> List.head)


preferredUrl : Media -> String
preferredUrl item =
    if item.variants.large /= "" then
        item.variants.large

    else if item.variants.original /= "" then
        item.variants.original

    else
        item.variants.medium


variantUrl : Media -> String -> String
variantUrl item variantName =
    case variantName of
        "thumbnail" ->
            item.variants.thumbnail

        "small" ->
            item.variants.small

        "medium" ->
            item.variants.medium

        "large" ->
            item.variants.large

        _ ->
            item.variants.original


scaledHeight : Int -> Media -> Int
scaledHeight displayWidth item =
    if item.width == 0 then
        0

    else
        round (toFloat displayWidth * toFloat item.height / toFloat item.width)


blockLabel : Block -> String
blockLabel block =
    case block of
        Paragraph _ ->
            "Paragraph"

        Heading level _ ->
            "Heading " ++ String.fromInt level

        Blockquote _ ->
            "Quote"

        BulletList _ ->
            "Bullet list"

        OrderedList _ ->
            "Numbered list"

        ImageBlock _ _ ->
            "Image"

        TableBlock _ ->
            "Table"

        HotelEmbed _ ->
            "Hotel"

        Preserved raw ->
            preservedType raw


preservedType : Encode.Value -> String
preservedType raw =
    Decode.decodeValue (Decode.field "type" Decode.string) raw |> Result.withDefault "content"


capitalize : String -> String
capitalize word =
    String.toUpper (String.left 1 word) ++ String.dropLeft 1 word


replaceAt : Int -> a -> List a -> List a
replaceAt index next items =
    List.indexedMap (\position item -> if position == index then next else item) items


removeAt : Int -> List a -> List a
removeAt index items =
    List.indexedMap Tuple.pair items |> List.filterMap (\( position, item ) -> if position == index then Nothing else Just item)


at : Int -> List a -> Maybe a
at index items =
    List.drop index items |> List.head
